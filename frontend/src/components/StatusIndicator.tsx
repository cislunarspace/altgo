import { useEffect, useState } from 'react';
import '../styles/components.css';

type Status = 'idle' | 'recording' | 'processing' | 'done';

interface StatusIndicatorProps {
  status: Status;
  size?: 'sm' | 'md' | 'lg';
}

const sizeMap = {
  sm: { dim: 48, stroke: 2 },
  md: { dim: 72, stroke: 3 },
  lg: { dim: 96, stroke: 4 },
};

const statusConfig = {
  idle: { color: '#52525b', label: '待命' },
  recording: { color: '#ef4444', label: '录音中' },
  processing: { color: '#f59e0b', label: '处理中' },
  done: { color: '#22c55e', label: '已完成' },
};

export function StatusIndicator({ status, size = 'md' }: StatusIndicatorProps) {
  const [pulseScale, setPulseScale] = useState(1);
  const [glowActive, setGlowActive] = useState(false);

  const { dim, stroke } = sizeMap[size];
  const center = dim / 2;
  const radius = (dim - stroke * 2) / 2;
  const config = statusConfig[status];

  useEffect(() => {
    if (status === 'recording') {
      const animate = () => {
        setPulseScale(1.05);
        setGlowActive(true);
        setTimeout(() => {
          setPulseScale(1);
          setGlowActive(false);
        }, 500);
      };
      animate();
      const interval = setInterval(animate, 800);
      return () => clearInterval(interval);
    }
  }, [status]);

  const glowSize = dim * 1.5;

  return (
    <div className="status-indicator">
      <div
        className="status-ring-container"
        style={{ width: dim, height: dim }}
      >
        <div
          className={`status-glow ${glowActive ? 'active' : ''}`}
          style={{
            width: glowSize,
            height: glowSize,
            color: config.color,
          }}
        />
        <svg
          className="status-svg"
          width={dim}
          height={dim}
          viewBox={`0 0 ${dim} ${dim}`}
          style={{
            transform: `scale(${pulseScale})`,
            transition: 'transform 0.3s cubic-bezier(0.34, 1.56, 0.64, 1)',
          }}
        >
          <circle
            className="status-ring-bg"
            cx={center}
            cy={center}
            r={radius}
            stroke="rgba(255,255,255,0.05)"
            strokeWidth={stroke}
          />
          <circle
            className={`status-ring-active ${status === 'processing' ? 'spinning' : ''}`}
            cx={center}
            cy={center}
            r={radius}
            stroke={config.color}
            strokeWidth={stroke}
            strokeDasharray={status === 'processing' ? `${radius * 1.5} ${radius * 5}` : undefined}
            style={{ transform: 'rotate(-90deg)', transformOrigin: 'center' }}
          />
        </svg>
      </div>
      <span
        className="status-label"
        style={{ color: config.color }}
      >
        {config.label}
      </span>
    </div>
  );
}
