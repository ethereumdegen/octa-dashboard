import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2, MessageSquarePlus, Send, X } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { kbApi } from "../api/kb";
import type { ChatMessage, SessionDetail } from "../types";

function isWaiting(detail: SessionDetail | undefined): boolean {
  const msgs = detail?.messages;
  if (!msgs || msgs.length === 0) return false;
  return msgs[msgs.length - 1].role === "user";
}

function Message({ msg }: { msg: ChatMessage }) {
  const isUser = msg.role === "user";
  return (
    <div className={`flex ${isUser ? "justify-end" : "justify-start"}`}>
      <div
        className={`max-w-[80%] rounded-2xl px-4 py-2.5 text-sm ${
          isUser ? "bg-gray-900 text-white" : "bg-white border border-gray-200 text-gray-900"
        }`}
      >
        {isUser ? (
          <span className="whitespace-pre-wrap">{msg.content}</span>
        ) : (
          <div className="prose prose-sm max-w-none prose-pre:bg-gray-100 prose-pre:text-gray-800">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{msg.content}</ReactMarkdown>
          </div>
        )}
        {msg.metadata?.tools_used && msg.metadata.tools_used.length > 0 && (
          <div className="mt-2 flex flex-wrap gap-1 border-t border-gray-100 pt-2">
            {msg.metadata.tools_used.map((t, i) => (
              <span key={i} className="rounded bg-gray-100 px-1.5 py-0.5 text-[10px] text-gray-500">
                {t}
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

export default function ChatTab({ kbId }: { kbId: string }) {
  const qc = useQueryClient();
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [input, setInput] = useState("");
  // The query the user just submitted, held until the optimistic user message
  // lands in the session cache (see send.onMutate). This drives the "Thinking…"
  // spinner from the instant Enter is pressed — including the window where a new
  // session is still being created and there's no `sessionId`/`detail` yet.
  const [pending, setPending] = useState<string | null>(null);
  // Explicit "a reply is outstanding" flag. This — not the cached message shape —
  // is what keeps `refetchInterval` polling. Deriving polling solely from
  // isWaiting(detail) is fragile: a slow in-flight getSession can land with the
  // pre-send state (empty/no trailing user message) and flip isWaiting false,
  // stopping polling forever so the reply only shows on a manual refresh. That
  // race is latency-sensitive — invisible locally, reproducible on Railway.
  const [awaitingReply, setAwaitingReply] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  const { data: sessions } = useQuery({
    queryKey: ["sessions", kbId],
    queryFn: () => kbApi.listSessions(kbId),
  });

  const { data: detail } = useQuery({
    queryKey: ["session", kbId, sessionId],
    queryFn: () => kbApi.getSession(kbId, sessionId!),
    enabled: !!sessionId,
    // Poll while we're awaiting a reply, or whenever the loaded session ends on
    // an unanswered user message (e.g. navigating back to an in-flight chat).
    refetchInterval: (q) =>
      awaitingReply || isWaiting(q.state.data as SessionDetail | undefined) ? 1500 : false,
  });

  // Clear the awaiting flag once the assistant's reply (or an error/timeout
  // message, which is also assistant-role) has actually landed from the server.
  useEffect(() => {
    const msgs = detail?.messages;
    if (awaitingReply && msgs?.length && msgs[msgs.length - 1].role === "assistant") {
      setAwaitingReply(false);
    }
  }, [detail?.messages, awaitingReply]);

  const newSession = useMutation({
    mutationFn: () => kbApi.createSession(kbId),
    onSuccess: (s) => {
      qc.invalidateQueries({ queryKey: ["sessions", kbId] });
      setSessionId(s.id);
    },
  });

  const send = useMutation({
    mutationFn: ({ sid, content }: { sid: string; content: string }) =>
      kbApi.sendMessage(kbId, sid, content),
    // Optimistically append the user message so the bubble shows instantly and
    // `isWaiting` flips true immediately — that's what starts `refetchInterval`
    // polling for the assistant reply. Relying on the post-send refetch to start
    // polling is racy: it can dedupe into the session query's own in-flight fetch
    // (which returns the pre-send state), so polling never engages and the reply
    // only appears on a hard refresh.
    onMutate: async ({ sid, content }) => {
      await qc.cancelQueries({ queryKey: ["session", kbId, sid] });
      const prev = qc.getQueryData<SessionDetail>(["session", kbId, sid]);
      const optimistic: ChatMessage = {
        id: `optimistic-${Date.now()}`,
        session_id: sid,
        role: "user",
        content,
        metadata: null,
        created_at: new Date().toISOString(),
      };
      qc.setQueryData<SessionDetail>(["session", kbId, sid], (old) =>
        old ? { ...old, messages: [...old.messages, optimistic] } : old,
      );
      // The optimistic bubble now stands in for the pending query — hand off so
      // the spinner stays continuous (isWaiting(detail) is now true) without
      // rendering the query twice.
      setPending(null);
      return { prev };
    },
    onError: (_err, { sid }, ctx) => {
      if (ctx?.prev) qc.setQueryData(["session", kbId, sid], ctx.prev);
      setPending(null);
      setAwaitingReply(false);
    },
    onSettled: (_data, _err, { sid }) => {
      qc.invalidateQueries({ queryKey: ["session", kbId, sid] });
      qc.invalidateQueries({ queryKey: ["sessions", kbId] });
    },
  });

  const deleteSession = useMutation({
    mutationFn: (sid: string) => kbApi.deleteSession(kbId, sid),
    onSuccess: (_data, sid) => {
      if (sid === sessionId) setSessionId(null);
      qc.invalidateQueries({ queryKey: ["sessions", kbId] });
    },
  });

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: "smooth" });
  }, [detail?.messages.length, send.isPending, pending]);

  const submit = async () => {
    const content = input.trim();
    if (!content) return;
    setInput("");
    // Show the user's bubble + spinner immediately, before we (possibly) await
    // session creation. send.onMutate clears `pending` once the optimistic
    // message takes over; `awaitingReply` keeps polling alive until the reply
    // lands, regardless of any stale getSession fetch that races in between.
    setPending(content);
    setAwaitingReply(true);
    let sid = sessionId;
    if (!sid) {
      const s = await kbApi.createSession(kbId, content.slice(0, 50));
      // Seed the session cache so the optimistic user message (added in
      // send.onMutate) has a detail object to append to before the first fetch.
      qc.setQueryData<SessionDetail>(["session", kbId, s.id], { session: s, messages: [] });
      qc.invalidateQueries({ queryKey: ["sessions", kbId] });
      setSessionId(s.id);
      sid = s.id;
    }
    send.mutate({ sid, content });
  };

  const waiting = awaitingReply || pending !== null || isWaiting(detail) || send.isPending;

  return (
    <div className="flex h-full gap-4">
      {/* Sessions sidebar */}
      <div className="w-56 shrink-0 overflow-y-auto rounded-xl border border-gray-200 bg-white p-2">
        <button
          onClick={() => newSession.mutate()}
          className="mb-2 flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm font-medium text-gray-700 hover:bg-gray-100"
        >
          <MessageSquarePlus size={16} /> New chat
        </button>
        {sessions?.map((s) => {
          const active = s.id === sessionId;
          return (
            <div
              key={s.id}
              className={`group flex items-center rounded-lg ${
                active ? "bg-gray-900 text-white" : "text-gray-600 hover:bg-gray-100"
              }`}
            >
              <button
                onClick={() => setSessionId(s.id)}
                className="min-w-0 flex-1 truncate px-3 py-2 text-left text-sm"
              >
                {s.title}
              </button>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  if (window.confirm("Delete this chat?")) deleteSession.mutate(s.id);
                }}
                title="Delete chat"
                className={`mr-1 shrink-0 rounded p-1 opacity-0 transition group-hover:opacity-100 ${
                  active ? "hover:bg-white/20" : "hover:bg-gray-200"
                }`}
              >
                <X size={14} />
              </button>
            </div>
          );
        })}
      </div>

      {/* Conversation */}
      <div className="flex flex-1 flex-col rounded-xl border border-gray-200 bg-gray-50">
        <div ref={scrollRef} className="flex-1 space-y-3 overflow-y-auto p-4">
          {!sessionId && pending === null ? (
            <div className="flex h-full items-center justify-center text-sm text-gray-400">
              Ask a question to start chatting with this knowledgebase.
            </div>
          ) : (
            <>
              {detail?.messages.map((m) => <Message key={m.id} msg={m} />)}
              {pending !== null && (
                <Message
                  msg={{
                    id: "pending",
                    session_id: sessionId ?? "",
                    role: "user",
                    content: pending,
                    metadata: null,
                    created_at: new Date().toISOString(),
                  }}
                />
              )}
              {waiting && (
                <div className="flex justify-start">
                  <div className="flex items-center gap-2 rounded-2xl border border-gray-200 bg-white px-4 py-2.5 text-sm text-gray-500">
                    <Loader2 size={14} className="animate-spin" /> Thinking…
                  </div>
                </div>
              )}
            </>
          )}
        </div>
        <div className="flex gap-2 border-t border-gray-200 p-3">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                submit();
              }
            }}
            rows={1}
            placeholder="Ask anything about your documents…"
            className="flex-1 resize-none rounded-lg border border-gray-300 px-3 py-2 text-sm focus:border-gray-900 focus:outline-none"
          />
          <button
            onClick={submit}
            disabled={!input.trim() || waiting}
            className="inline-flex items-center rounded-lg bg-gray-900 px-4 text-white disabled:opacity-50"
          >
            <Send size={16} />
          </button>
        </div>
      </div>
    </div>
  );
}
