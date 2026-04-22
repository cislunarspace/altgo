import React from 'react';

const features = [
  {
    title: '长按即录',
    description: '按住右 Alt 键说话，松开自动处理。无需切换窗口，无需打开应用。',
    icon: '🎙️',
  },
  {
    title: '双击连续录',
    description: '双击右 Alt 进入连续模式，适合长篇发言。再次单击停止。',
    icon: '♾️',
  },
  {
    title: '本地 + 云端 ASR',
    description: '支持本地 whisper.cpp（无需联网）和 OpenAI Whisper API 两种转写引擎。',
    icon: '🧠',
  },
  {
    title: 'LLM 润色',
    description: '四档润色强度：不润色、修正标点、改善语法、结构化重写。支持 OpenAI / DeepSeek / Anthropic / Ollama。',
    icon: '✨',
  },
  {
    title: '跨平台',
    description: 'Linux（X11 / Wayland）与 Windows 使用子进程集成系统工具；发布与 CI 以 Linux 为主，Windows 为附带支持。',
    icon: '💻',
  },
  {
    title: '剪贴板与悬浮窗',
    description: '成功后写入系统剪贴板并弹出悬浮窗核对；可在悬浮窗内再次复制。Windows 可选光标注入（见配置）。',
    icon: '📋',
  },
];

export default function Cards() {
  return (
    <div style={{
      display: 'grid',
      gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))',
      gap: '1.5rem',
      marginTop: '2rem',
    }}>
      {features.map((f) => (
        <div key={f.title} style={{
          padding: '1.5rem',
          borderRadius: '12px',
          border: '1px solid var(--ifm-color-emphasis-300)',
          background: 'var(--ifm-background-surface-color)',
        }}>
          <div style={{fontSize: '2rem', marginBottom: '0.5rem'}}>{f.icon}</div>
          <h3 style={{margin: '0 0 0.5rem'}}>{f.title}</h3>
          <p style={{margin: 0, color: 'var(--ifm-color-emphasis-700)', fontSize: '0.95rem'}}>
            {f.description}
          </p>
        </div>
      ))}
    </div>
  );
}
