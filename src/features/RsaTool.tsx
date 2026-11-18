import { useEffect, useMemo, useState, type ReactNode } from "react";
import CodeEditor from "../components/CodeEditor";
import { CopyBtn, Pane, PrimaryBtn, Segmented, Select, copyText } from "../components/ui";
import { IconLock } from "../components/icons";
import {
  api,
  type RsaInputEncoding,
  type RsaKeyInfo,
  type RsaKeyPair,
  type RsaOutcome,
  type RsaOutputEncoding,
  type RsaScheme,
  type RsaSpec,
} from "../lib/api";

type Mode = "keys" | "encrypt" | "decrypt";
type KeyFormat = "spki" | "pkcs8" | "pkcs1";

const te = new TextEncoder();

const BITS = [
  { value: "2048", label: "2048 位" },
  { value: "3072", label: "3072 位" },
  { value: "4096", label: "4096 位（慢）" },
];

const SCHEMES: { value: RsaScheme; label: string }[] = [
  { value: "oaep-sha256", label: "OAEP-SHA256" },
  { value: "oaep-sha1", label: "OAEP-SHA1" },
  { value: "pkcs1v15", label: "PKCS#1 v1.5" },
  { value: "hybrid", label: "混合 ikt3" },
];

const PRIV_FORMATS: { value: KeyFormat; label: string }[] = [
  { value: "pkcs8", label: "PKCS#8" },
  { value: "pkcs1", label: "PKCS#1" },
];

const PUB_FORMATS: { value: KeyFormat; label: string }[] = [
  { value: "spki", label: "SPKI" },
  { value: "pkcs1", label: "PKCS#1" },
];

/** 与 crates/ikt-core/src/rsa.rs 的 max_bytes 保持一致，只用于界面上的长度提示。 */
function maxFor(scheme: RsaScheme, keyBits: number): number {
  const k = keyBits / 8;
  if (scheme === "oaep-sha256") return k - 66;
  if (scheme === "oaep-sha1") return k - 42;
  if (scheme === "pkcs1v15") return k - 11;
  return 0;
}

function Field({ label, title, children }: { label: string; title?: string; children: ReactNode }) {
  return (
    <div className="flex shrink-0 items-center gap-1.5" title={title}>
      <span className="whitespace-nowrap text-[11px] text-faint">{label}</span>
      {children}
    </div>
  );
}

function ErrorBox({ message }: { message: string }) {
  return (
    <div className="p-4">
      <div className="rounded-[8px] border border-[#e6c3bb] bg-danger-soft p-3">
        <div className="flex items-center gap-1.5 text-[12px] font-medium text-danger">
          <IconLock className="h-3.5 w-3.5" />
          无法完成
        </div>
        <p className="mt-1.5 text-[12px] leading-relaxed text-[#7d2e24]">{message}</p>
      </div>
    </div>
  );
}

/** 左栏：上面是密钥，下面是待处理的内容。RSA 的输入天然包含这两块。 */
function KeyedPane({
  keyLabel,
  keyValue,
  onKeyChange,
  keyInfo,
  keyPlaceholder,
  onUseGenerated,
  onClear,
  bodyLabel,
  bodyValue,
  onBodyChange,
  bodyPlaceholder,
}: {
  keyLabel: string;
  keyValue: string;
  onKeyChange: (v: string) => void;
  keyInfo: RsaKeyInfo | null;
  keyPlaceholder: string;
  onUseGenerated?: () => void;
  onClear: () => void;
  bodyLabel: string;
  bodyValue: string;
  onBodyChange: (v: string) => void;
  bodyPlaceholder: string;
}) {
  return (
    <section className="flex min-w-0 flex-1 flex-col bg-panel">
      <div className="flex h-[38px] shrink-0 items-center justify-between border-b border-line pl-3.5 pr-2">
        <div className="flex min-w-0 items-center gap-2 text-[11px] font-medium tracking-wide text-muted">
          <span className="whitespace-nowrap">{keyLabel}</span>
        </div>
        <div className="flex shrink-0 items-center gap-0.5">
          {onUseGenerated ? (
            <button
              onClick={onUseGenerated}
              className="whitespace-nowrap rounded-[6px] px-2 py-1 text-[11.5px] text-muted transition-colors hover:bg-[#f2f0eb] hover:text-ink"
            >
              用生成的
            </button>
          ) : null}
          <button
            onClick={onClear}
            className="whitespace-nowrap rounded-[6px] px-2 py-1 text-[11.5px] text-muted transition-colors hover:bg-[#f2f0eb] hover:text-ink"
          >
            清空
          </button>
          <CopyBtn text={keyValue} />
        </div>
      </div>

      <div className="h-[168px] shrink-0">
        <CodeEditor
          value={keyValue}
          onChange={onKeyChange}
          language="text"
          placeholder={keyPlaceholder}
        />
      </div>

      <div className="flex h-[26px] shrink-0 items-center justify-between border-y border-line bg-[#faf9f6] pl-3.5 pr-2 text-[10.5px] text-faint">
        <span className="truncate">
          {keyValue.trim()
            ? keyInfo
              ? [
                  keyInfo.kind === "private" ? "私钥（内含公钥）" : "公钥",
                  `${keyInfo.bits} 位`,
                  `e=${keyInfo.exponent}`,
                  keyInfo.fingerprintOpenssh,
                ]
                  .filter(Boolean)
                  .join(" · ")
              : "无法识别这段密钥"
            : "还没填密钥"}
        </span>
        {keyInfo ? (
          <button
            onClick={() => copyText(keyInfo.fingerprintSha256)}
            title="复制 SHA-256 指纹（Hex）"
            className="shrink-0 rounded-[4px] px-1 text-[10px] hover:text-muted"
          >
            复制指纹
          </button>
        ) : null}
      </div>

      <div className="flex h-[30px] shrink-0 items-center gap-2 border-b border-line pl-3.5">
        <span className="whitespace-nowrap text-[10.5px] font-medium text-muted">{bodyLabel}</span>
      </div>
      <div className="min-h-0 flex-1">
        <CodeEditor
          value={bodyValue}
          onChange={onBodyChange}
          language="text"
          placeholder={bodyPlaceholder}
        />
      </div>
    </section>
  );
}

export default function RsaTool() {
  const [mode, setMode] = useState<Mode>("keys");
  const [bits, setBits] = useState("2048");
  const [pair, setPair] = useState<RsaKeyPair | null>(null);
  const [pubPem, setPubPem] = useState("");
  const [privPem, setPrivPem] = useState("");
  const [privFormat, setPrivFormat] = useState<KeyFormat>("pkcs8");
  const [pubFormat, setPubFormat] = useState<KeyFormat>("spki");
  const [scheme, setScheme] = useState<RsaScheme>("oaep-sha256");
  const [outEnc, setOutEnc] = useState<RsaOutputEncoding>("base64");
  const [inEnc, setInEnc] = useState<RsaInputEncoding>("auto");
  const [plaintext, setPlaintext] = useState("");
  const [ciphertext, setCiphertext] = useState("");
  const [result, setResult] = useState<RsaOutcome | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [generating, setGenerating] = useState(false);
  const [pubInfo, setPubInfo] = useState<RsaKeyInfo | null>(null);
  const [privInfo, setPrivInfo] = useState<RsaKeyInfo | null>(null);

  const spec: RsaSpec = useMemo(
    () => ({ scheme, outputEncoding: outEnc, inputEncoding: inEnc }),
    [scheme, outEnc, inEnc],
  );

  const generate = async (size: number) => {
    if (generating) return;
    setGenerating(true);
    setErr(null);
    try {
      const created = await api.rsaGenerateKeypair(size);
      setPair(created);
      setPubPem(created.publicPem);
      setPrivPem(created.privatePem);
      setPrivFormat("pkcs8");
      setPubFormat("spki");
    } catch (e) {
      setErr(String(e));
    } finally {
      setGenerating(false);
    }
  };

  // 密钥一改就拿去校验：位数、指纹都从 Rust 侧算，前端不重复实现
  useEffect(() => {
    let alive = true;
    const text = pubPem.trim();
    if (!text) {
      setPubInfo(null);
      return;
    }
    const timer = setTimeout(() => {
      api
        .rsaInspect(text)
        .then((info) => alive && setPubInfo(info))
        .catch(() => alive && setPubInfo(null));
    }, 220);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [pubPem]);

  useEffect(() => {
    let alive = true;
    const text = privPem.trim();
    if (!text) {
      setPrivInfo(null);
      return;
    }
    const timer = setTimeout(() => {
      api
        .rsaInspect(text)
        .then((info) => alive && setPrivInfo(info))
        .catch(() => alive && setPrivInfo(null));
    }, 220);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [privPem]);

  // 密钥或内容一改就作废上一轮结果，避免拿旧结果去反推
  useEffect(() => {
    setResult(null);
    setErr(null);
    setBusy(false);
  }, [mode, spec, pubPem, privPem, plaintext, ciphertext]);

  const run = async () => {
    if (mode === "keys" || busy) return;
    setBusy(true);
    try {
      const r =
        mode === "encrypt"
          ? await api.rsaEncrypt(spec, pubPem, plaintext)
          : await api.rsaDecrypt(spec, privPem, ciphertext);
      setResult(r);
      setErr(null);
    } catch (e) {
      setResult(null);
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  const useGenerated = (which: "pub" | "priv") => {
    if (!pair) return;
    if (which === "pub") setPubPem(pair.publicPem);
    else setPrivPem(pair.privatePem);
  };

  const privText = pair
    ? privPem.trim() === pair.privatePem.trim()
      ? privFormat === "pkcs1"
        ? pair.privatePkcs1Pem
        : pair.privatePem
      : privPem
    : privPem;

  const pubText = pair
    ? pubPem.trim() === pair.publicPem.trim()
      ? pubFormat === "pkcs1"
        ? pair.publicPkcs1Pem
        : pair.publicPem
      : pubPem
    : pubPem;

  const plainBytes = te.encode(plaintext).length;
  const limit = pubInfo ? maxFor(scheme, pubInfo.bits) : 0;
  const overLimit = limit > 0 && plainBytes > limit;
  const hybridInput = ciphertext.trim().startsWith("ikt3.");

  const keyStatus = pubInfo
    ? `${pubInfo.kind === "private" ? "私钥" : "公钥"} ${pubInfo.bits} 位${
        pubInfo.fingerprintOpenssh ? ` · ${pubInfo.fingerprintOpenssh.slice(0, 18)}…` : ""
      }`
    : generating
      ? "正在本机生成密钥…"
      : "未就绪";

  const swap = () => {
    if (!result?.output) return;
    if (mode === "encrypt") {
      setCiphertext(result.output);
      setMode("decrypt");
    } else if (mode === "decrypt") {
      setPlaintext(result.output);
      setMode("encrypt");
    }
  };

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-[44px] shrink-0 items-center gap-2.5 overflow-x-auto border-b border-line bg-page px-5">
        <Segmented
          value={mode}
          onChange={setMode}
          options={[
            { value: "keys", label: "密钥对" },
            { value: "encrypt", label: "加密" },
            { value: "decrypt", label: "解密" },
          ]}
        />

        <span className="hidden h-[15px] w-px shrink-0 bg-line min-[1000px]:block" />

        <span className="hidden whitespace-nowrap text-[11px] text-faint min-[1000px]:inline">
          {mode === "keys"
            ? pair
              ? `${pair.bits} 位 · e=${pair.exponent}`
              : generating
                ? "生成中…"
                : ""
            : keyStatus}
        </span>

        <div className="ml-auto flex shrink-0 items-center gap-2.5">
          {mode !== "keys" ? (
            <PrimaryBtn onClick={() => void run()} disabled={busy}>
              {busy ? "计算中…" : mode === "encrypt" ? "加密" : "解密"}
            </PrimaryBtn>
          ) : null}
          <button
            onClick={() => setMode("keys")}
            className="whitespace-nowrap rounded-[6px] px-2 py-1 text-[11.5px] text-muted transition-colors hover:bg-[#f2f0eb] hover:text-ink"
            title="回到密钥对页面更换密钥"
          >
            密钥管理
          </button>
          {mode !== "keys" ? (
            <button
              onClick={swap}
              disabled={!result?.output}
              className="whitespace-nowrap rounded-[6px] px-2 py-1 text-[11.5px] text-muted transition-colors hover:bg-[#f2f0eb] hover:text-ink disabled:opacity-40"
            >
              {mode === "encrypt" ? "结果送去解密 →" : "结果送去加密 →"}
            </button>
          ) : null}
        </div>
      </div>

      <div className="flex min-h-[42px] shrink-0 flex-wrap items-center gap-x-3.5 gap-y-2 border-b border-line bg-page px-5 py-[6px]">
        {mode === "keys" ? (
          <>
            <Field label="长度">
              <Select value={bits} onChange={setBits} options={BITS} />
            </Field>
            <button
              onClick={() => void generate(Number(bits))}
              className="shrink-0 whitespace-nowrap rounded-[7px] bg-accent px-3 py-[4px] text-[12px] font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
              disabled={generating}
            >
              {generating ? "生成中…" : "生成密钥对"}
            </button>
            {pair ? (
              <Field label="SHA-256 指纹" title="公钥 SPKI DER 的 SHA-256，用于证书固定 / 校验分发是否一致">
                <button
                  onClick={() => copyText(`${pair.fingerprintOpenssh}\n${pair.fingerprintSha256}`)}
                  className="max-w-[300px] truncate rounded-[6px] border border-dashed border-line px-2 py-[3px] font-mono text-[10.5px] text-muted hover:border-line-strong hover:text-ink"
                >
                  {pair.fingerprintOpenssh}
                </button>
              </Field>
            ) : null}
            <span className="ml-auto whitespace-nowrap font-mono text-[11px] text-faint">
              {pair ? `${pair.elapsedMs} ms` : ""}
            </span>
          </>
        ) : mode === "encrypt" ? (
          <>
            <Field label="方案" title="对接 Java / Go 时先确认对方用的是 OAEP 还是 PKCS#1 v1.5">
              <Select value={scheme} onChange={setScheme} options={SCHEMES} />
            </Field>
            <Field label="输出">
              <Segmented
                value={outEnc}
                onChange={setOutEnc}
                options={[
                  { value: "base64", label: "Base64" },
                  { value: "hex", label: "Hex" },
                ]}
              />
            </Field>
            <span
              className={`whitespace-nowrap text-[11px] ${overLimit ? "text-danger" : "text-faint"}`}
            >
              {scheme === "hybrid"
                ? "不限长度"
                : limit > 0
                  ? `单条上限 ${limit} 字节 · 当前 ${plainBytes} 字节`
                  : "无公钥"}
            </span>
          </>
        ) : (
          <>
            <Field label="方案" title={hybridInput ? "ikt3 密文自带方案标识，这里的选择会被忽略" : undefined}>
              <Select value={scheme} onChange={setScheme} options={SCHEMES} disabled={hybridInput} />
            </Field>
            <Field label="输入">
              <Segmented
                value={inEnc}
                onChange={setInEnc}
                options={[
                  { value: "auto", label: "自动" },
                  { value: "base64", label: "Base64" },
                  { value: "hex", label: "Hex" },
                ]}
              />
            </Field>
            <span className="whitespace-nowrap font-mono text-[11px] text-faint">
              {hybridInput ? "ikt3" : result ? `${result.payloadBytes} B` : ""}
            </span>
          </>
        )}
      </div>

      {mode === "keys" ? (
        <div className="flex min-h-0 flex-1 gap-px bg-line">
          <Pane
            label={<>私钥</>}
            actions={
              <>
                <Select value={privFormat} onChange={setPrivFormat} options={PRIV_FORMATS} />
                <CopyBtn text={privText} />
              </>
            }
          >
            <CodeEditor
              value={privText}
              language="text"
              readOnly
              placeholder={generating ? "正在生成…" : "点击上方「生成密钥对」"}
            />
          </Pane>

          <Pane
            label={<>公钥</>}
            actions={
              <>
                <Select value={pubFormat} onChange={setPubFormat} options={PUB_FORMATS} />
                <CopyBtn text={pubText} />
              </>
            }
          >
            <CodeEditor
              value={pubText}
              language="text"
              readOnly
              placeholder={generating ? "正在生成…" : "点击上方「生成密钥对」"}
            />
          </Pane>
        </div>
      ) : (
        <div className="flex min-h-0 flex-1 gap-px bg-line">
          <KeyedPane
            keyLabel={mode === "encrypt" ? "公钥" : "私钥"}
            keyValue={mode === "encrypt" ? pubPem : privPem}
            onKeyChange={mode === "encrypt" ? setPubPem : setPrivPem}
            keyInfo={mode === "encrypt" ? pubInfo : privInfo}
            keyPlaceholder={
              mode === "encrypt"
                ? "-----BEGIN PUBLIC KEY-----\n…"
                : "-----BEGIN PRIVATE KEY-----\n…"
            }
            onUseGenerated={() => useGenerated(mode === "encrypt" ? "pub" : "priv")}
            onClear={() => (mode === "encrypt" ? setPubPem("") : setPrivPem(""))}
            bodyLabel={mode === "encrypt" ? "明文" : "密文"}
            bodyValue={mode === "encrypt" ? plaintext : ciphertext}
            onBodyChange={mode === "encrypt" ? setPlaintext : setCiphertext}
            bodyPlaceholder={mode === "encrypt" ? "要加密的内容…" : "粘贴密文…"}
          />

          <Pane
            label={
              <>
                {mode === "encrypt" ? "密文" : "明文"}
                {busy ? <span className="font-normal text-faint">计算中…</span> : null}
              </>
            }
            actions={<CopyBtn text={result?.output ?? ""} />}
          >
            {err ? (
              <ErrorBox message={err} />
            ) : (
              <div className="flex h-full flex-col">
                {result ? (
                  <div className="flex shrink-0 items-center gap-2 border-b border-[#efe2c6] bg-[#fdf8ec] px-3.5 py-1.5 text-[11px] leading-[16px] text-[#8a6a34]">
                    <span className="whitespace-nowrap font-medium">{result.scheme}</span>
                    <span className="text-[#b59a68]">·</span>
                    <span className="whitespace-nowrap">{result.keyBits} 位</span>
                    <span className="text-[#b59a68]">·</span>
                    <span className="whitespace-nowrap">{result.elapsedMs} ms</span>
                    {result.notes.length ? (
                      <span className="truncate text-[#a3854f]">{result.notes.join(" · ")}</span>
                    ) : null}
                  </div>
                ) : null}
                <div className="min-h-0 flex-1">
                  <CodeEditor
                    value={result?.output ?? ""}
                    language="text"
                    readOnly
                    placeholder={
                      mode === "encrypt" ? "密文会出现在这里…" : "明文会出现在这里…"
                    }
                  />
                </div>
              </div>
            )}
          </Pane>
        </div>
      )}
    </div>
  );
}
