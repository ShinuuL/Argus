/** O mascote do Argus: bichinho de um olho só. Usado no widget da status bar e na página de Extras. */
import { type ReactNode } from "react";
import { type PetMood } from "./mood";

/** Olho (e boca) de cada humor. O olho fica em (32, 33), raio 12. */
function face(mood: PetMood): ReactNode {
  switch (mood) {
    case "sleeping":
      return (
        <>
          <circle className="argo-lid" cx="32" cy="33" r="12" />
          <path className="argo-line" d="M22 34 Q32 42 42 34" />
        </>
      );
    case "happy":
      return (
        <>
          <circle className="argo-lid" cx="32" cy="33" r="12" />
          <path className="argo-line" d="M22 37 Q32 25 42 37" />
          <path className="argo-ink" d="M27 49 Q32 55 37 49 Z" />
        </>
      );
    case "working":
      return (
        <>
          <circle className="argo-eye" cx="32" cy="33" r="12" />
          <g className="argo-pupil">
            <circle className="argo-ink" cx="32" cy="33" r="5.5" />
            <circle className="argo-glint" cx="34" cy="31" r="1.6" />
          </g>
          <path className="argo-line" d="M29 50 h6" />
        </>
      );
    case "sweating":
      return (
        <>
          <circle className="argo-eye" cx="32" cy="33" r="12" />
          <circle className="argo-ink" cx="32" cy="33" r="3.2" />
          <ellipse className="argo-ink" cx="32" cy="50.5" rx="3" ry="2.2" />
        </>
      );
    case "worried":
      return (
        <>
          <circle className="argo-eye" cx="32" cy="33" r="12" />
          <circle className="argo-ink" cx="29" cy="36" r="4.2" />
          <circle className="argo-glint" cx="30.5" cy="34.5" r="1.2" />
          <path className="argo-line" d="M21 21 L41 16" />
          <path className="argo-line" d="M28 52 Q32 48 36 52" />
        </>
      );
  }
}

export function Mascot({ mood, size = 22 }: { mood: PetMood; size?: number }) {
  return (
    <svg className="argo-svg" viewBox="0 0 64 64" width={size} height={size} aria-hidden="true">
      <ellipse className="argo-shadow" cx="32" cy="60.5" rx="15" ry="2.5" />
      <g className="argo-body">
        {/* antenas */}
        <path className="argo-stalk" d="M26 16 Q23 8 19 7" />
        <path className="argo-stalk" d="M38 16 Q41 8 45 7" />
        <circle className="argo-tip" cx="19" cy="7" r="2.6" />
        <circle className="argo-tip" cx="45" cy="7" r="2.6" />
        {/* pés e corpo */}
        <ellipse className="argo-foot" cx="23" cy="57" rx="5.5" ry="2.6" />
        <ellipse className="argo-foot" cx="41" cy="57" rx="5.5" ry="2.6" />
        <path
          className="argo-fill"
          d="M32 13 C46 13 54 24 54 38 C54 51 45 57 32 57 C19 57 10 51 10 38 C10 24 18 13 32 13 Z"
        />
        <circle className="argo-cheek" cx="15.5" cy="45" r="2.8" />
        <circle className="argo-cheek" cx="48.5" cy="45" r="2.8" />
        {face(mood)}
        {mood === "sweating" && <path className="argo-drop" d="M51 15 q3.2 5 0 7.2 q-3.2 -2.2 0 -7.2 Z" />}
      </g>
      {mood === "sleeping" && <path className="argo-z" d="M47 5 h7 l-7 8 h7" />}
      {mood === "happy" && (
        <>
          <path className="argo-spark" d="M8 6 L9.5 10.5 L14 12 L9.5 13.5 L8 18 L6.5 13.5 L2 12 L6.5 10.5 Z" />
          <path className="argo-spark late" d="M57 16 L58 19 L61 20 L58 21 L57 24 L56 21 L53 20 L56 19 Z" />
        </>
      )}
    </svg>
  );
}
