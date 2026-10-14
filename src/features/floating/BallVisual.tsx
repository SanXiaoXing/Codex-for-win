import { ballInfo, clampPercent } from "../../lib/usage-format";

/** 实际悬浮球和转场中的球使用相同的内容，避免最后一帧换形。 */
export function BallVisual({ info }: { info: ReturnType<typeof ballInfo> }) {
  const ringLength = 2 * Math.PI * 26;
  const percent = info.remaining === null ? 0 : clampPercent(info.remaining);
  return (
    <div className={`ball-circle ${info.tone}`}>
      <svg className="ball-ring" viewBox="0 0 60 60" aria-hidden="true">
        <circle className="track" cx="30" cy="30" r="26" />
        <circle className="value" cx="30" cy="30" r="26" transform="rotate(-90 30 30)"
          strokeDasharray={ringLength} strokeDashoffset={ringLength * (1 - percent / 100)} />
      </svg>
      <div className="ball-core">
        {info.remaining === null ? <strong className="ball-empty">--</strong> : <strong>{info.remaining}<i>%</i></strong>}
        <span>{info.mark}</span>
      </div>
    </div>
  );
}

