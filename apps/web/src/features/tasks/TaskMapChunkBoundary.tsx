import { Component, type ReactNode } from 'react';
import { Button } from '../../components/ui/button';
export class TaskMapChunkBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  render() {
    if (!this.state.failed) return this.props.children;
    return <section className="paper-banner banner-error" data-testid="task-map-chunk-error" role="alert"><h2>依赖图暂不可用</h2><p>页面资源加载失败。</p><Button onClick={() => window.location.reload()}>重试</Button></section>;
  }
}
