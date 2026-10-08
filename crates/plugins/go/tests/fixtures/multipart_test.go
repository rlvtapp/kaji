package upload
import("bytes"; "context"; "io"; "mime"; "mime/multipart"; "net/http"; "net/http/httptest"; "strings"; "sync"; "testing"; "time")
func TestMultipartUpload(t *testing.T) {
    var mutex sync.Mutex
    captures:=map[string][][]byte{}
    server:=httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter,r *http.Request){
        data,err:=io.ReadAll(r.Body);if err!=nil{t.Error(err)}
        mutex.Lock();captures[r.URL.Path]=append(captures[r.URL.Path],data);attempt:=len(captures[r.URL.Path]);mutex.Unlock()
        if strings.HasPrefix(r.Header.Get("Content-Type"),"multipart/") {
            _,parameters,err:=mime.ParseMediaType(r.Header.Get("Content-Type"));if err!=nil{t.Error(err)}
            reader:=multipart.NewReader(bytes.NewReader(data),parameters["boundary"])
            names:=[]string{};values:=[][]byte{};filenames:=[]string{};types:=[]string{}
            for {part,err:=reader.NextPart();if err==io.EOF{break};if err!=nil{t.Error(err);break};value,err:=io.ReadAll(part);if err!=nil{t.Error(err)};names=append(names,part.FormName());values=append(values,value);filenames=append(filenames,part.FileName());types=append(types,part.Header.Get("Content-Type"))}
            if len(names)!=5||names[0]!="file"||filenames[0]!="é.bin"||!bytes.Equal(values[0],[]byte{0,255,1})||string(values[1])!="é"||string(values[2])!="0"||string(values[3])!="false"||types[3]!="application/json"||names[4]!="items[]"||string(values[4])!="second"{t.Errorf("bad MIME %#v %#v %#v %#v",names,values,filenames,types)}
        } else if string(data)!="{\"value\":false}" {t.Errorf("lost JSON alternative %s",data)}
        if attempt==1{w.WriteHeader(503)}else{w.WriteHeader(204)}
    }));defer server.Close()
    client,err:=NewClient(ClientConfig{BaseURL:server.URL,HTTPClient:server.Client(),Retry:&RetryConfig{MaxAttempts:2,InitialDelay:time.Millisecond}});if err!=nil{t.Fatal(err)}
    original:=[]byte{0,255,1};body:=&PoolsterMultipartBody{}
    body.AddFile("file","é.bin","application/octet-stream",original);original[0]=99
    body.AddText("name","é");body.AddText("items[]","0");if err:=body.AddJSON("flag",false);err!=nil{t.Fatal(err)};body.AddText("items[]","second")
    if err:=client.UploadFile(context.Background(),&UploadFileRequest{Body:body});err!=nil{t.Fatal(err)}
    if err:=client.CreateFile(context.Background(),&CreateFileRequest{Body:body});err==nil{t.Fatal("unsafe POST retried")}
    mutex.Lock();a:=captures["/upload"];b:=captures["/create"];mutex.Unlock()
    if len(a)!=2||!bytes.Equal(a[0],a[1])||len(b)!=1{t.Fatal("retry bytes or safety")}
    if err:=client.UploadFile(context.Background(),&UploadFileRequest{Body:map[string]any{"value":false}});err!=nil{t.Fatal(err)}
    invalid:=&PoolsterMultipartBody{Parts:[]PoolsterMultipartPart{{Name:"bad\r\ninjection",Data:[]byte("x")}}}
    if _,_,err:=invalid.encode();err==nil{t.Fatal("header injection accepted")}
    huge:=&PoolsterMultipartBody{Parts:[]PoolsterMultipartPart{{Name:"huge",Data:make([]byte,(64<<20)+1)}}}
    if _,_,err:=huge.encode();err==nil{t.Fatal("body bound missing")}
}
