/**
 * Realce de sintaxe mínimo e síncrono para os blocos de código do chat.
 * Recebe o código CRU e devolve HTML escapado, com <span class="hl-*">.
 * Heurístico (não é um parser): comentários, strings, números e palavras-chave
 * das linguagens mais comuns. Cores em highlight.css.
 */
import "./highlight.css";

const ESC: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };
const esc = (s: string) => s.replace(/[&<>"']/g, (c) => ESC[c]);

const KEYWORDS = new Set(
  (
    "abstract as async await break case catch class const continue crate def default defer del do elif else enum export " +
    "extends extern false final finally fn for from func function go if impl implements import in instanceof interface " +
    "is lambda let loop match mod module mut new nil none null of package pass private protected pub public raise ref " +
    "return self Self static struct super switch this throw throws trait true try type typeof undefined unsafe use var " +
    "void where while with yield True False None and or not echo then fi done esac local select from where join"
  ).split(" "),
);

const HASH_COMMENT = /^(py|python|sh|bash|shell|zsh|ps1|powershell|rb|ruby|yaml|yml|toml|r|perl|dockerfile|make|makefile|conf|ini)$/i;
const NO_HL = /^(text|txt|plain|plaintext|md|markdown|log|diff)$/i;

/** HTML realçado (já escapado) para `code` na linguagem `lang` (pode ser ""). */
export function highlightCode(code: string, lang = ""): string {
  if (NO_HL.test(lang) || code.length > 200_000) return esc(code);
  const hash = HASH_COMMENT.test(lang);
  const sqlish = /^(sql|lua|hs|haskell)$/i.test(lang);
  const parts = [
    String.raw`\/\*[\s\S]*?(?:\*\/|$)`,
    hash ? "#[^\n]*" : String.raw`\/\/[^\n]*`,
    sqlish ? "--[^\n]*" : "(?!)",
    String.raw`"(?:\\.|[^"\\\n])*"?`,
    String.raw`'(?:\\.|[^'\\\n])*'?`,
    String.raw`\x60(?:\\.|[^\x60\\])*\x60?`,
    String.raw`\b(?:0x[\da-fA-F_]+|\d[\d_]*(?:\.\d+)?(?:e[+-]?\d+)?)\b`,
    String.raw`[A-Za-z_$][\w$]*`,
  ];
  const re = new RegExp(parts.map((p) => `(${p})`).join("|"), "g");
  let out = "";
  let last = 0;
  for (let m = re.exec(code); m; m = re.exec(code)) {
    if (m[0] === "") { re.lastIndex++; continue; }
    out += esc(code.slice(last, m.index));
    const t = esc(m[0]);
    let cls = "";
    if (m[1] || m[2] || m[3]) cls = "hl-com";
    else if (m[4] || m[5] || m[6]) cls = "hl-str";
    else if (m[7]) cls = "hl-num";
    else if (m[8]) {
      if (KEYWORDS.has(m[8]) || (sqlish && KEYWORDS.has(m[8].toLowerCase()))) cls = "hl-kw";
      else if (code[re.lastIndex] === "(") cls = "hl-fn";
      else if (/^[A-Z]/.test(m[8])) cls = "hl-type";
    }
    out += cls ? `<span class="${cls}">${t}</span>` : t;
    last = re.lastIndex;
  }
  return out + esc(code.slice(last));
}
