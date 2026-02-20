import { useEffect, useState } from 'react'
import { Brain, Activity, BarChart3, Target, Zap, TrendingUp, Database, History } from 'lucide-react'
import type { MLMetrics, MLModelInfo, MLPrediction, MLState } from '../types/ui'

interface MLPanelProps {
  mlState: MLState | null
  mlMetrics: MLMetrics | null
  mlPrediction: MLPrediction | null
}

function ModelWeightBar({ model, maxWeight }: { model: MLModelInfo; maxWeight: number }) {
  const percentage = maxWeight > 0 ? (model.weight / maxWeight) * 100 : 0
  const accuracyColor = model.accuracy >= 0.55 ? '#22c55e' : model.accuracy >= 0.50 ? '#eab308' : '#ef4444'

  return (
    <div className="model-weight-item">
      <div className="model-weight-header">
        <span className="model-name">{model.name}</span>
        <span className="model-stats">
          {(model.weight * 100).toFixed(0)}% | {(model.accuracy * 100).toFixed(1)}%
        </span>
      </div>
      <div className="model-weight-bar-container">
        <div
          className="model-weight-bar"
          style={{ width: `${percentage}%`, backgroundColor: accuracyColor }}
        />
      </div>
      <span className={`model-status model-status-${model.status}`}>{model.status}</span>
    </div>
  )
}

function formatTimeAgo(timestamp: number): string {
  const seconds = Math.floor((Date.now() - timestamp) / 1000)
  if (seconds < 60) return `${seconds}s ago`
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`
  return `${Math.floor(seconds / 3600)}h ago`
}

export function MLPanel({ mlState, mlMetrics, mlPrediction }: MLPanelProps) {
  const [recentPredictions, setRecentPredictions] = useState<MLPrediction[]>([])

  useEffect(() => {
    if (mlPrediction) {
      setRecentPredictions(prev => [mlPrediction, ...prev].slice(0, 5))
    }
  }, [mlPrediction])

  if (!mlState || !mlMetrics) {
    return (
      <div className="ml-panel-empty">
        <Brain size={32} className="ml-icon" />
        <p>ML Engine initializing...</p>
        <span className="ml-hint">Waiting for first predictions</span>
      </div>
    )
  }

  const maxWeight = Math.max(...mlMetrics.ensembleWeights.map(m => m.weight), 0.01)
  const isActive = mlState.enabled

  return (
    <div className="ml-panel">
      {/* Header */}
      <div className="ml-header">
        <div className="ml-title">
          <Brain size={18} className={isActive ? 'ml-icon-active' : 'ml-icon'} />
          <span>ML Engine v{mlState.version}</span>
          <span className={`ml-status-badge ${isActive ? 'ml-status-active' : 'ml-status-inactive'}`}>
            {isActive ? 'Active' : 'Inactive'}
          </span>
        </div>
        <div className="ml-model-type">
          {mlState.modelType}
        </div>
      </div>

      {/* Metrics Grid */}
      <div className="ml-metrics-grid">
        <div className="ml-metric-card">
          <Activity size={14} />
          <span className="ml-metric-label">Accuracy</span>
          <span className={`ml-metric-value ${mlMetrics.accuracy >= 0.55 ? 'text-positive' : 'text-warning'}`}>
            {(mlMetrics.accuracy * 100).toFixed(1)}%
          </span>
        </div>

        <div className="ml-metric-card">
          <Target size={14} />
          <span className="ml-metric-label">Win Rate</span>
          <span className={`ml-metric-value ${mlMetrics.winRate >= 0.55 ? 'text-positive' : 'text-warning'}`}>
            {(mlMetrics.winRate * 100).toFixed(1)}%
          </span>
        </div>

        <div className="ml-metric-card">
          <BarChart3 size={14} />
          <span className="ml-metric-label">Predictions</span>
          <span className="ml-metric-value">{mlMetrics.totalPredictions}</span>
        </div>

        <div className="ml-metric-card">
          <TrendingUp size={14} />
          <span className="ml-metric-label">Correct</span>
          <span className="ml-metric-value text-positive">{mlMetrics.correctPredictions}</span>
        </div>

        <div className="ml-metric-card">
          <TrendingUp size={14} style={{ transform: 'rotate(180deg)' }} />
          <span className="ml-metric-label">Loss</span>
          <span className="ml-metric-value text-negative">{mlMetrics.incorrectPredictions}</span>
        </div>

        <div className="ml-metric-card">
          <History size={14} />
          <span className="ml-metric-label">Epoch</span>
          <span className="ml-metric-value">{mlMetrics.epoch}</span>
        </div>

        <div className="ml-metric-card">
          <Database size={14} />
          <span className="ml-metric-label">Dataset</span>
          <span className="ml-metric-value">{mlMetrics.datasetSize}</span>
        </div>
      </div>

      {/* Ensemble Weights */}
      <div className="ml-section">
        <div className="ml-section-title">
          <Zap size={14} />
          <span>Ensemble Weights</span>
        </div>
        <div className="ml-model-weights">
          {mlMetrics.ensembleWeights.map((model, idx) => (
            <ModelWeightBar key={idx} model={model} maxWeight={maxWeight} />
          ))}
        </div>
      </div>

      {/* Recent Predictions */}
      {recentPredictions.length > 0 && (
        <div className="ml-section">
          <div className="ml-section-title">
            <Target size={14} />
            <span>Recent Predictions</span>
          </div>
          <div className="ml-predictions-list">
            {recentPredictions.map((pred, idx) => (
              <div key={idx} className="ml-prediction-item">
                <div className="ml-prediction-header">
                  <span className="ml-prediction-asset">{pred.asset}_{pred.timeframe}</span>
                  <span className={`ml-prediction-direction ml-direction-${pred.direction.toLowerCase()}`}>
                    {pred.direction}
                  </span>
                  <span className="ml-prediction-confidence">{(pred.confidence * 100).toFixed(0)}%</span>
                </div>
                <div className="ml-prediction-details">
                  <span>Prob Up: {(pred.probUp * 100).toFixed(0)}%</span>
                  <span className="ml-prediction-model">{pred.modelName}</span>
                  <span className="ml-prediction-time">{formatTimeAgo(pred.timestamp)}</span>
                </div>
                {pred.featuresTriggered.length > 0 && (
                  <div className="ml-prediction-features">
                    {pred.featuresTriggered.slice(0, 3).map((feat, i) => (
                      <span key={i} className="ml-feature-tag">{feat}</span>
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Footer */}
      <div className="ml-footer">
        <span>Last update: {formatTimeAgo(mlMetrics.timestamp)}</span>
      </div>
    </div>
  )
}
