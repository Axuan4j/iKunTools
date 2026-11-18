import { useEffect, useMemo, useState, type ReactNode } from "react";
import CodeEditor from "../components/CodeEditor";
import { CopyBtn, Pane, PrimaryBtn, Segmented, Select, copyText } from "../components/ui";
import { IconKey, IconLock } from "../components/icons";
import { api, type AesEncoding, type AesMode, type AesOutcome, type AesPadding, type AesSpec } from "../lib/api";

type Work = "encrypt" | "decrypt";
type KeyMode = "password" | "raw";
type IvMode = "random" | "custom";

const MODES: { value: AesMode; label: string }[] = [
  { value: "gcm", label: "GCM" },
  { value: "cbc", label: "CBC" },
  { value: "ctr", label: "CTR" },
  { value: "ecb", label: "ECB" },
];

const PADDINGS: { value: AesPadding; label: string }[] = [
  { value: "pkcs7", label: "PKCS#7" },
  { value: "zero", label: "Zero" },
  { value: "none", label: "无填充" },
];

const ENCODINGS: { value: AesEncoding; label: string }[] = [
  { value: "utf8", label: "UTF-8" },
  { value: "hex", label: "Hex" },
  { value: "base64", label: "Base64" },
];

const ivHint = (mode: AesMode) => (mode === "gcm" ? "12 字节（24 hex）或 16 字节" : "16 字节（32 hex）");
const randomIvHex = (mode: AesMode) =>
  Array.from(crypto.getRandomValues(new Uint8Array(mode === "gcm" ? 12 : 16)), (b) =>
    b.toString(16).padStart(2, "0"),
  ).join("");
const isStream = (mode: AesMode) => mode === "gcm" || mode === "ctr";

function Field({ label, title, children }: { label: string; title?: string; children: ReactNode }) {
  return (
    <div className="flex shrink-0 items-center gap-1.5" title={title}>
      <span className="whitespace-nowrap text-[11px] text-faint">{label}</span>
      {children}
    </div>
  );
}

export default function AesTool() {
  const [work, setWork] = useState<Work>("encrypt");
  const [cipher, setCipher] = useState<AesMode>("gcm");
  const [keyMode, setKeyMode] = useState<KeyMode>("password");
  const [secret, setSecret] = useState("");
  const [showSecret, setShowSecret] = useState(false);
  const [keyEncoding, setKeyEncoding] = useState<AesEncoding>("utf8");
  const [ivMode, setIvMode] = useState<IvMode>("random");
  const [ivValue, setIvValue] = useState("");
  const [ivEncoding, setIvEncoding] = useState<AesEncoding>("hex");
  const [padding, setPadding] = useState<AesPadding>("pkcs7");
  const [ctEncoding, setCtEncoding] = useState<"base64" | "hex">("base64");
  const [input, setInput] = useState("");
  const [result, setResult] = useState<AesOutcome | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const effectivePadding: AesPadding = isStream(cipher) ? "none" : padding;

  const spec: AesSpec = useMemo(
    () => ({
      mode: cipher,
      keyMode,
      secret,
      keyEncoding,
      ivMode,
      iv: ivValue,
      ivEncoding,
      padding: effectivePadding,
      outputEncoding: ctEncoding,
    }),
    [cipher, keyMode, secret, keyEncoding, ivMode, ivValue, ivEncoding, effectivePadding, ctEncoding],
  );

  // 裸密文没有 salt，只能用「原始密钥 + 自定 IV」解密，切过去时顺手把选项调好
  useEffect(() => {
    if (work === "decrypt" && keyMode === "raw" && ivMode === "random") setIvMode("custom");
  }, [work, keyMode, ivMode]);

  // 参数或输入一改就作废上一轮结果，避免拿旧密文去解密
  useEffect(() => {
    setResult(null);
    setErr(null);
  }, [work, spec, input]);

  const run = async () => {
    if (busy) return;
    setBusy(true);
    try {
      const r = work === "encrypt" ? await api.aesEncrypt(spec, input) : await api.aesDecrypt(spec, input);
      setResult(r);
      setErr(null);
    } catch (e) {
      setResult(null);
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  const changeKeyMode = (next: KeyMode) => {
    setKeyMode(next);
    setKeyEncoding(next === "raw" ? "hex" : "utf8");
  };

  const changeIvMode = (next: IvMode) => {
    setIvMode(next);
    if (next === "custom" && !ivValue.trim() && work === "encrypt") {
      setIvEncoding("hex");
      setIvValue(randomIvHex(cipher));
    }
  };

  const swap = () => {
    if (!result?.output) return;
    setInput(result.output);
    if (!result.ivEmbedded && result.ivHex) {
      setIvMode("custom");
      setIvEncoding("hex");
      setIvValue(result.ivHex);
    }
    setWork(work === "encrypt" ? "decrypt" : "encrypt");
  };

  const algoText = result
    ? `${result.algorithm} · ${result.keyBits} 位 · ${result.padding === "none" ? "无填充" : result.padding.toUpperCase()}`
    : `${keyMode === "password" ? "PBKDF2-SHA256 120k" : `原始密钥 ${keyEncoding.toUpperCase()}`}`;

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-[44px] shrink-0 items-center gap-2.5 overflow-x-auto border-b border-line bg-page px-5">
        <Segmented
          value={work}
          onChange={setWork}
          options={[
            { value: "encrypt", label: "加密" },
            { value: "decrypt", label: "解密" },
          ]}
        />

        <span className="h-[15px] w-px bg-line" />

        <Segmented
          value={keyMode}
          onChange={changeKeyMode}
          options={[
            { value: "password", label: "口令" },
            { value: "raw", label: "密钥" },
          ]}
        />

        <label className="flex shrink-0 items-center gap-1.5 rounded-[7px] border border-line bg-panel px-2 py-[4px] focus-within:border-line-strong">
          <IconKey className="h-[13px] w-[13px] shrink-0 text-faint" />
          <input
            type={showSecret || keyMode === "raw" ? "text" : "password"}
            value={secret}
            onChange={(e) => setSecret(e.target.value)}
            placeholder={keyMode === "raw" ? "原始密钥" : "口令"}
            spellCheck={false}
            className={`bg-transparent font-mono text-[11.5px] placeholder:text-faint ${
              keyMode === "raw" ? "w-[228px]" : "w-[138px]"
            }`}
          />
          {keyMode === "password" ? (
            <button
              onClick={() => setShowSecret((v) => !v)}
              className="text-[10px] text-faint hover:text-muted"
              title={showSecret ? "隐藏" : "显示"}
            >
              {showSecret ? "隐藏" : "显示"}
            </button>
          ) : null}
        </label>

        {keyMode === "raw" ? (
          <Select value={keyEncoding} onChange={setKeyEncoding} options={ENCODINGS} />
        ) : null}

        <div className="ml-auto flex items-center gap-2.5">
          <span className="hidden max-w-[230px] truncate text-[11px] text-faint min-[1120px]:inline">{algoText}</span>
          <PrimaryBtn onClick={() => void run()} disabled={busy}>
            {busy ? "计算中…" : work === "encrypt" ? "加密" : "解密"}
          </PrimaryBtn>
          <button
            onClick={swap}
            disabled={!result?.output}
            className="shrink-0 whitespace-nowrap rounded-[6px] px-2 py-1 text-[11.5px] text-muted transition-colors hover:bg-[#f2f0eb] hover:text-ink disabled:opacity-40"
          >
            结果回填并反向 →
          </button>
        </div>
      </div>

      <div className="flex min-h-[42px] shrink-0 flex-wrap items-center gap-x-3.5 gap-y-2 border-b border-line bg-page px-5 py-[6px]">
        <Field label="模式">
          <Select value={cipher} onChange={setCipher} options={MODES} />
        </Field>

        <Field
          label="填充"
          title={isStream(cipher) ? "GCM / CTR 是流式模式，不需要填充" : "ZeroPadding 会把结尾的 0x00 当作填充去掉"}
        >
          <Select
            value={effectivePadding}
            onChange={setPadding}
            disabled={isStream(cipher)}
            options={PADDINGS}
          />
        </Field>

        <Field label="IV">
          {cipher === "ecb" ? (
            <span className="text-[11px] text-faint">不使用</span>
          ) : (
            <>
              <Segmented
                value={ivMode}
                onChange={changeIvMode}
                options={[
                  { value: "random", label: "随机" },
                  { value: "custom", label: "自定" },
                ]}
              />
              {ivMode === "custom" ? (
                <>
                  <input
                    value={ivValue}
                    onChange={(e) => setIvValue(e.target.value)}
                    placeholder={ivHint(cipher)}
                    spellCheck={false}
                    className="w-[232px] rounded-[6px] border border-line bg-panel px-2 py-[3px] font-mono text-[10.5px] placeholder:text-faint focus:border-line-strong"
                  />
                  <Select value={ivEncoding} onChange={setIvEncoding} options={ENCODINGS} />
                </>
              ) : (
                <button
                  onClick={() => result?.ivHex && copyText(result.ivHex)}
                  title={result?.ivHex ? "复制本次使用的 IV" : "每次加密随机生成"}
                  className="w-[232px] truncate rounded-[6px] border border-dashed border-line px-2 py-[3px] text-left font-mono text-[10.5px] text-faint hover:border-line-strong hover:text-muted"
                >
                  {result?.ivHex || "未生成"}
                </button>
              )}
            </>
          )}
        </Field>

        <Field label="输出">
          <Segmented
            value={ctEncoding}
            onChange={setCtEncoding}
            options={[
              { value: "base64", label: "Base64" },
              { value: "hex", label: "Hex" },
            ]}
          />
        </Field>
      </div>

      <div className="flex min-h-0 flex-1 gap-px bg-line">
        <Pane
          label={<>{work === "encrypt" ? "明文" : "密文"}</>}
        >
          <CodeEditor
            value={input}
            onChange={setInput}
            language="text"
            placeholder={work === "encrypt" ? "要加密的内容…" : "粘贴密文…"}
          />
        </Pane>

        <Pane
          label={
            <>
              {work === "encrypt" ? "密文" : "明文"}
              {busy ? <span className="font-normal text-faint">计算中…</span> : null}
            </>
          }
          actions={
            <>
              {result && !result.ivEmbedded && result.ivHex ? (
                <CopyBtn text={result.ivHex} label="IV" />
              ) : null}
              <CopyBtn text={result?.output ?? ""} />
            </>
          }
        >
          {err ? (
            <div className="p-4">
              <div className="rounded-[8px] border border-[#e6c3bb] bg-danger-soft p-3">
                <div className="flex items-center gap-1.5 text-[12px] font-medium text-danger">
                  <IconLock className="h-3.5 w-3.5" />
                  无法完成
                </div>
                <p className="mt-1.5 text-[12px] leading-relaxed text-[#7d2e24]">{err}</p>
              </div>
            </div>
          ) : (
            <div className="flex h-full flex-col">
              {result?.notes.length ? (
                <div className="shrink-0 border-b border-[#efe2c6] bg-[#fdf8ec] px-3.5 py-1.5 text-[11px] leading-[16px] text-[#8a6a34]">
                  {result.notes.join(" · ")}
                </div>
              ) : null}
              <div className="min-h-0 flex-1">
                <CodeEditor
                  value={result?.output ?? ""}
                  language="text"
                  readOnly
                  placeholder="结果会出现在这里…"
                />
              </div>
            </div>
          )}
        </Pane>
      </div>
    </div>
  );
}
