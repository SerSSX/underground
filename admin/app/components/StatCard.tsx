// Dashboard stat card: icon + value + label, with an optional trend chip.
// (Sparkline/7-day series is a fast-follow once the API exposes a time series.)

interface Props {
  icon: string;
  value: string | number;
  label: string;
  trend?: { dir: "up" | "down" | "flat"; text: string };
}

export default function StatCard({ icon, value, label, trend }: Props) {
  return (
    <div className="statcard">
      <div className="top">
        <span className="ico" aria-hidden>{icon}</span>
        {trend && <span className={`trend ${trend.dir}`}>{trend.dir === "up" ? "▲" : trend.dir === "down" ? "▼" : "•"} {trend.text}</span>}
      </div>
      <div className="v">{value}</div>
      <div className="l">{label}</div>
    </div>
  );
}
