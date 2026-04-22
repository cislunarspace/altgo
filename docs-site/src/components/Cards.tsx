import React from 'react';
import {
  Mic,
  Infinity,
  Brain,
  Sparkles,
  Monitor,
  ClipboardCopy,
  type LucideIcon,
} from 'lucide-react';
import styles from './Cards.module.css';

interface FeatureItem {
  title: string;
  description: string;
  icon: LucideIcon;
}

const features: FeatureItem[] = [
  {
    title: '长按即录',
    description: '按住右 Alt 键说话，松开自动处理。无需切换窗口，无需打开应用。',
    icon: Mic,
  },
  {
    title: '双击连续录',
    description: '双击右 Alt 进入连续模式，适合长篇发言。再次单击停止。',
    icon: Infinity,
  },
  {
    title: '本地 + 云端 ASR',
    description: '支持本地 whisper.cpp（无需联网）和 OpenAI Whisper API 两种转写引擎。',
    icon: Brain,
  },
  {
    title: 'LLM 润色',
    description: '四档润色强度：不润色、修正标点、改善语法、结构化重写。支持 OpenAI / DeepSeek / Anthropic / Ollama。',
    icon: Sparkles,
  },
  {
    title: '跨平台',
    description: 'Linux（X11 / Wayland）与 Windows 使用子进程集成系统工具；发布与 CI 以 Linux 为主。',
    icon: Monitor,
  },
  {
    title: '剪贴板与悬浮窗',
    description: '成功后写入系统剪贴板并弹出悬浮窗核对；可在悬浮窗内再次复制。Windows 可选光标注入。',
    icon: ClipboardCopy,
  },
];

export default function Cards(): JSX.Element {
  return (
    <div className={styles.grid}>
      {features.map((f) => {
        const Icon = f.icon;
        return (
          <div key={f.title} className={styles.card}>
            <div className={styles.iconWrap}>
              <Icon size={22} strokeWidth={2} />
            </div>
            <h3 className={styles.title}>{f.title}</h3>
            <p className={styles.desc}>{f.description}</p>
          </div>
        );
      })}
    </div>
  );
}
