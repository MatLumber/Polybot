import { Component, type ErrorInfo, type ReactNode } from 'react'

interface ChartErrorBoundaryProps {
  children: ReactNode
}

interface ChartErrorBoundaryState {
  hasError: boolean
}

export class ChartErrorBoundary extends Component<
  ChartErrorBoundaryProps,
  ChartErrorBoundaryState
> {
  state: ChartErrorBoundaryState = { hasError: false }

  static getDerivedStateFromError(): ChartErrorBoundaryState {
    return { hasError: true }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('ChartErrorBoundary', error, info)
  }

  render() {
    if (this.state.hasError) {
      return <div className="empty-state">Chart unavailable. Waiting for next stream sync.</div>
    }
    return this.props.children
  }
}
