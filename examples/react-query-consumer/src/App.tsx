import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Notes } from "@example/notes";
import { useListNotes } from "@example/notes/react-query/react-query";

const api = new Notes({ baseUrl: "http://localhost:4010" });
const queryClient = new QueryClient();

function NotesList() {
  const notes = useListNotes({ client: api.transport });
  if (notes.isPending) return <p>Loading…</p>;
  if (notes.isError) return <p>Could not load notes.</p>;
  return <ul>{notes.data?.data.map((note) => <li key={note.id}>{note.body}</li>)}</ul>;
}

export function App() {
  return <QueryClientProvider client={queryClient}><NotesList /></QueryClientProvider>;
}
