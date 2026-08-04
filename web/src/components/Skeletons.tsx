/**
 * Fixed-dimension loading placeholders.
 *
 * A bare `Loading...` string collapses the layout to one line and then expands
 * it again when data arrives, producing a full-page layout shift on every cold
 * load. These skeletons reuse the real layout classes so the boxes occupy the
 * space the content will occupy.
 *
 * Each skeleton is `role="status"` with `aria-busy`, so assistive technology
 * announces that content is loading rather than reading an empty region.
 */

const BOARD_COLUMN_COUNT = 5;
const BOARD_CARDS_PER_COLUMN = 4;

export function BoardSkeleton({ label = "Loading board" }: { label?: string }) {
  return (
    <div className="view" role="status" aria-busy="true" aria-live="polite">
      <span className="visually-hidden">{label}</span>
      <div className="skeleton-toolbar" aria-hidden="true">
        <div className="skeleton-block skeleton-block--title" />
        <div className="skeleton-block skeleton-block--field" />
      </div>
      <div className="columns" aria-hidden="true">
        {Array.from({ length: BOARD_COLUMN_COUNT }, (_, column) => (
          <div className="column" key={column}>
            <h4>
              <span className="skeleton-block skeleton-block--label" />
            </h4>
            {Array.from({ length: BOARD_CARDS_PER_COLUMN }, (_, card) => (
              <div className="card skeleton-card" key={card} />
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}

const DASHBOARD_KPI_COUNT = 4;
const DASHBOARD_CHART_COUNT = 4;

export function DashboardSkeleton({ label = "Loading dashboard" }: { label?: string }) {
  return (
    <div className="view" role="status" aria-busy="true" aria-live="polite">
      <span className="visually-hidden">{label}</span>
      <div className="skeleton-kpis" aria-hidden="true">
        {Array.from({ length: DASHBOARD_KPI_COUNT }, (_, index) => (
          <div className="skeleton-kpi" key={index} />
        ))}
      </div>
      <div className="skeleton-charts" aria-hidden="true">
        {Array.from({ length: DASHBOARD_CHART_COUNT }, (_, index) => (
          <div className="skeleton-chart" key={index} />
        ))}
      </div>
    </div>
  );
}

/** Generic full-view placeholder for routes without a bespoke skeleton. */
export function ViewSkeleton({ label = "Loading" }: { label?: string }) {
  return (
    <div className="view" role="status" aria-busy="true" aria-live="polite">
      <span className="visually-hidden">{label}</span>
      <div className="skeleton-charts" aria-hidden="true">
        <div className="skeleton-chart" />
        <div className="skeleton-chart" />
      </div>
    </div>
  );
}
