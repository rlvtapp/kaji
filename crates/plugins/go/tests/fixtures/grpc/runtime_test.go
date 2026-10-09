package storev1_test

import (
	"context"
	pb "example.com/poolster/grpc/storev1"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/metadata"
	"google.golang.org/grpc/status"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/timestamppb"
	"io"
	"net"
	"strings"
	"testing"
	"time"
)

type store struct{ pb.UnimplementedStoreServer }

func (store) Get(ctx context.Context, request *pb.Item) (*pb.Item, error) {
	if request.Name == "error" {
		return nil, status.Error(codes.InvalidArgument, "bad item")
	}
	if request.Name == "wait" {
		<-ctx.Done()
		return nil, status.FromContextError(ctx.Err()).Err()
	}
	if incoming, _ := metadata.FromIncomingContext(ctx); incoming.Get("caller")[0] != "poolster" {
		return nil, status.Error(codes.Unauthenticated, "missing metadata")
	}
	if err := grpc.SetHeader(ctx, metadata.Pairs("checked", "yes")); err != nil {
		return nil, err
	}
	return request, nil
}
func (store) Watch(request *pb.Item, stream pb.Store_WatchServer) error {
	if request.Name == "partial" {
		if err := stream.Send(&pb.Item{Name: "first"}); err != nil {
			return err
		}
		return status.Error(codes.Unavailable, "stream failed after data")
	}
	for _, name := range []string{request.Name + "-one", request.Name + "-two"} {
		if err := stream.Send(&pb.Item{Name: name}); err != nil {
			return err
		}
	}
	return nil
}
func (store) Upload(stream pb.Store_UploadServer) error {
	var names []string
	for {
		item, err := stream.Recv()
		if err == io.EOF {
			return stream.SendAndClose(&pb.Item{Name: strings.Join(names, ",")})
		}
		if err != nil {
			return err
		}
		names = append(names, item.Name)
	}
}
func (store) Chat(stream pb.Store_ChatServer) error {
	for {
		item, err := stream.Recv()
		if err == io.EOF {
			return nil
		}
		if err != nil {
			return err
		}
		if err := stream.Send(item); err != nil {
			return err
		}
	}
}
func TestGeneratedPackageAgainstLocalServer(t *testing.T) {
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	server := grpc.NewServer()
	pb.RegisterStoreServer(server, store{})
	go func() { _ = server.Serve(listener) }()
	t.Cleanup(server.Stop)
	conn, err := grpc.NewClient(listener.Addr().String(), grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = conn.Close() })
	client := pb.NewStoreClient(conn)
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	ctx = metadata.NewOutgoingContext(ctx, metadata.Pairs("caller", "poolster"))
	note := ""
	request := &pb.Item{Name: "item", Note: &note, Value: &pb.Item_Count{Count: 7}, Labels: map[string]string{"owner": "test"}, State: pb.Item_READY, CreatedAt: timestamppb.New(time.Unix(1, 0))}
	var header metadata.MD
	response, err := client.Get(ctx, request, grpc.Header(&header))
	if err != nil {
		t.Fatal(err)
	}
	if !proto.Equal(request, response) || response.Note == nil || header.Get("checked")[0] != "yes" {
		t.Fatalf("unary presence/metadata changed: %v %v", response, header)
	}
	_, err = client.Get(ctx, &pb.Item{Name: "error"})
	if status.Code(err) != codes.InvalidArgument {
		t.Fatalf("expected status error: %v", err)
	}
	watch, err := client.Watch(ctx, &pb.Item{Name: "event"})
	if err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"event-one", "event-two"} {
		item, err := watch.Recv()
		if err != nil || item.Name != name {
			t.Fatalf("server stream: %v %v", item, err)
		}
	}
	if _, err = watch.Recv(); err != io.EOF {
		t.Fatalf("server stream end: %v", err)
	}
	upload, err := client.Upload(ctx)
	if err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"a", "b"} {
		if err := upload.Send(&pb.Item{Name: name}); err != nil {
			t.Fatal(err)
		}
	}
	summary, err := upload.CloseAndRecv()
	if err != nil || summary.Name != "a,b" {
		t.Fatalf("client stream: %v %v", summary, err)
	}
	chat, err := client.Chat(ctx)
	if err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"left", "right"} {
		if err := chat.Send(&pb.Item{Name: name}); err != nil {
			t.Fatal(err)
		}
		item, err := chat.Recv()
		if err != nil || item.Name != name {
			t.Fatalf("bidi stream: %v %v", item, err)
		}
	}
	if err := chat.CloseSend(); err != nil {
		t.Fatal(err)
	}
	if _, err = chat.Recv(); err != io.EOF {
		t.Fatalf("bidi stream end: %v", err)
	}
	deadline, stop := context.WithTimeout(context.Background(), 40*time.Millisecond)
	defer stop()
	_, err = client.Get(deadline, &pb.Item{Name: "wait"})
	if status.Code(err) != codes.DeadlineExceeded {
		t.Fatalf("deadline: %v", err)
	}
	canceled, stop := context.WithCancel(context.Background())
	stop()
	_, err = client.Get(canceled, &pb.Item{Name: "wait"})
	if status.Code(err) != codes.Canceled {
		t.Fatalf("cancellation: %v", err)
	}
	partial, err := client.Watch(ctx, &pb.Item{Name: "partial"})
	if err != nil {
		t.Fatal(err)
	}
	first, err := partial.Recv()
	if err != nil || first.Name != "first" {
		t.Fatalf("partial stream data: %v %v", first, err)
	}
	if _, err = partial.Recv(); status.Code(err) != codes.Unavailable {
		t.Fatalf("partial stream error: %v", err)
	}
	streamCtx, stopStream := context.WithCancel(ctx)
	active, err := client.Chat(streamCtx)
	if err != nil {
		t.Fatal(err)
	}
	if err := active.Send(&pb.Item{Name: "before-cancel"}); err != nil {
		t.Fatal(err)
	}
	if _, err := active.Recv(); err != nil {
		t.Fatal(err)
	}
	stopStream()
	if _, err := active.Recv(); status.Code(err) != codes.Canceled {
		t.Fatalf("stream cancellation: %v", err)
	}

}
