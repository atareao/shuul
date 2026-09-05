# Charts Revamp — Implementation Plan

## Objetivo

Reemplazar la página de charts actual (1 Line + 2 Pie) por una página con 2 tabs, cada uno con 4 charts, añadiendo top_methods y top_paths tanto en backend como frontend.

## Arquitectura

Dos pipelines independientes:
- **WAF** (`POST /api/v1/shuul`) — ForwardAuth, intercept/match/allow/deny
- **Jail** (`POST /api/v1/report`) — Rate limiter post-factum

Los nuevos datos (`top_methods`, `top_paths`) se recolectan en `StatsCollector` desde los puntos de llamada existentes (`record_blocked`, `record_allowed`) y se sirven vía dos nuevos endpoints HTTP. El frontend consume todos los endpoints en paralelo y organiza los charts en dos tabs: "Evolution" y "Rankings".

## Tareas

### Tarea 1: StatsCollector — añadir top_methods y top_paths

**Archivos:**
- Modificar: `backend/src/models/stats.rs`

- [ ] **Paso 1:** Añadir dos nuevos campos `Mutex<HashMap<String, u64>>` a la struct `StatsCollector`:
      ```rust
      top_methods: Mutex<HashMap<String, u64>>,
      top_paths: Mutex<HashMap<String, u64>>,
      ```

- [ ] **Paso 2:** Inicializar ambos en el constructor (`StatsCollector::new()`):
      ```rust
      top_methods: Mutex::new(HashMap::new()),
      top_paths: Mutex::new(HashMap::new()),
      ```

- [ ] **Paso 3:** Actualizar `record_blocked()` para aceptar `method: Option<&str>` y `path: Option<&str>`:
      ```rust
      pub fn record_blocked(&self, method: Option<&str>, path: Option<&str>) {
          // ... lógica existente ...
          if let Some(m) = method {
              let mut methods = self.top_methods.lock().unwrap();
              *methods.entry(m.to_string()).or_insert(0) += 1;
          }
          if let Some(p) = path {
              let mut paths = self.top_paths.lock().unwrap();
              *paths.entry(p.to_string()).or_insert(0) += 1;
          }
      }
      ```

- [ ] **Paso 4:** Actualizar `record_allowed()` para aceptar `method: Option<&str>` y `path: Option<&str>`:
      ```rust
      pub fn record_allowed(&self, method: Option<&str>, path: Option<&str>) {
          // ... lógica existente ...
          if let Some(m) = method {
              let mut methods = self.top_methods.lock().unwrap();
              *methods.entry(m.to_string()).or_insert(0) += 1;
          }
          if let Some(p) = path {
              let mut paths = self.top_paths.lock().unwrap();
              *paths.entry(p.to_string()).or_insert(0) += 1;
          }
      }
      ```

- [ ] **Paso 5:** Actualizar `StatsSnapshot` para incluir `top_methods: HashMap<String, u64>` y `top_paths: HashMap<String, u64>`.

- [ ] **Paso 6:** En `snapshot()` (dentro de `StatsCollector`), incluir ambos campos en el snapshot:
      ```rust
      top_methods: self.top_methods.lock().unwrap().clone(),
      top_paths: self.top_paths.lock().unwrap().clone(),
      ```

- [ ] **Paso 7:** En `load_snapshot()`, restaurar ambos campos:
      ```rust
      *self.top_methods.lock().unwrap() = snapshot.top_methods;
      *self.top_paths.lock().unwrap() = snapshot.top_paths;
      ```

- [ ] **Paso 8:** Añadir getters públicos:
      ```rust
      pub fn get_top_methods(&self) -> Vec<(String, u64)> {
          self.top_methods.lock().unwrap().iter().map(|(k, v)| (k.clone(), *v)).collect()
      }
      pub fn get_top_paths(&self) -> Vec<(String, u64)> {
          self.top_paths.lock().unwrap().iter().map(|(k, v)| (k.clone(), *v)).collect()
      }
      ```

### Tarea 2: Stats HTTP endpoints — añadir /stats/top_methods y /stats/top_paths

**Archivos:**
- Modificar: `backend/src/http/stats.rs`

- [ ] **Paso 1:** Añadir endpoint `GET /stats/top_methods`:
      ```rust
      async fn get_top_methods(State(state): State<AppState>) -> Result<Json<Vec<(String, i32, f32)>>, AppError> {
          let methods = state.stats_collector.get_top_methods();
          let total: u64 = methods.iter().map(|(_, c)| c).sum();
          let result: Vec<(String, i32, f32)> = methods
              .into_iter()
              .map(|(m, c)| {
                  let pct = if total > 0 { (c as f32 / total as f32) * 100.0 } else { 0.0 };
                  (m, c as i32, pct)
              })
              .collect();
          Ok(Json(result))
      }
      ```

- [ ] **Paso 2:** Añadir endpoint `GET /stats/top_paths`:
      ```rust
      async fn get_top_paths(State(state): State<AppState>) -> Result<Json<Vec<(String, i32, f32)>>, AppError> {
          let paths = state.stats_collector.get_top_paths();
          let total: u64 = paths.iter().map(|(_, c)| c).sum();
          let result: Vec<(String, i32, f32)> = paths
              .into_iter()
              .map(|(p, c)| {
                  let pct = if total > 0 { (c as f32 / total as f32) * 100.0 } else { 0.0 };
                  (p, c as i32, pct)
              })
              .collect();
          Ok(Json(result))
      }
      ```

- [ ] **Paso 3:** Registrar ambas rutas en la función de registro de rutas de stats:
      ```rust
      .route("/stats/top_methods", get(get_top_methods))
      .route("/stats/top_paths", get(get_top_paths))
      ```

### Tarea 3: Callers — pasar method y path a record_blocked / record_allowed

**Archivos:**
- Modificar: `backend/src/http/shuul.rs`
- Modificar: `backend/src/http/report.rs`

- [ ] **Paso 1:** En `shuul.rs`, localizar TODAS las llamadas a `record_blocked()` y `record_allowed()`. Añadir `request.method.as_deref()` y `request.path.as_deref()` como argumentos:
      ```rust
      // Antes:
      stats.record_blocked();
      // Después:
      stats.record_blocked(request.method.as_deref(), request.path.as_deref());

      // Antes:
      stats.record_allowed();
      // Después:
      stats.record_allowed(request.method.as_deref(), request.path.as_deref());
      ```

- [ ] **Paso 2:** En `report.rs`, localizar TODAS las llamadas a `record_blocked()` y `record_allowed()`. Añadir `method` y `path` desde el request:
      ```rust
      // Antes:
      stats.record_blocked();
      // Después:
      stats.record_blocked(Some(&report.method), Some(&report.path));
      ```
      Nota: en report.rs el request llega como `ReportRequest` con campos `method` y `path`.

### Tarea 4: Frontend — nuevo componente antd_column.tsx

**Archivos:**
- Crear: `frontend/src/components/charts/antd_column.tsx`

- [ ] **Paso 1:** Crear archivo que re-exporte `Column` desde `@ant-design/charts`:
      ```typescript
      export { Column } from '@ant-design/charts';
      ```

### Tarea 5: Frontend — nuevo componente summary_cards.tsx

**Archivos:**
- Crear: `frontend/src/components/charts/summary_cards.tsx`

- [ ] **Paso 1:** Crear componente con 4 cards en fila (Row/Col de Ant Design):
      ```typescript
      import React from 'react';
      import { Card, Statistic, Row, Col } from 'antd';

      interface SummaryCardsProps {
          total: number;
          allowed: number;
          blocked: number;
      }

      const SummaryCards: React.FC<SummaryCardsProps> = ({ total, allowed, blocked }) => {
          const blockRate = total > 0 ? ((blocked / total) * 100).toFixed(1) + '%' : '0.0%';
          return (
              <Row gutter={16} style={{ marginBottom: 24 }}>
                  <Col span={6}>
                      <Card><Statistic title="Total Requests" value={total} /></Card>
                  </Col>
                  <Col span={6}>
                      <Card><Statistic title="Allowed" value={allowed} /></Card>
                  </Col>
                  <Col span={6}>
                      <Card><Statistic title="Blocked" value={blocked} /></Card>
                  </Col>
                  <Col span={6}>
                      <Card><Statistic title="Block Rate" value={blockRate} /></Card>
                  </Col>
              </Row>
          );
      };

      export default SummaryCards;
      ```

### Tarea 6: Frontend — nuevo componente evolution_stacked.tsx

**Archivos:**
- Crear: `frontend/src/components/charts/evolution_stacked.tsx`

- [ ] **Paso 1:** Crear stacked column chart:
      ```typescript
      import React, { Suspense } from 'react';
      import { Column } from './antd_column';

      interface EvolutionStackedProps {
          data: Array<{ category: string; time: string; requests: number }>;
          isDarkMode: boolean;
      }

      const EvolutionStacked: React.FC<EvolutionStackedProps> = ({ data, isDarkMode }) => {
          const config = {
              data,
              isStack: true,
              xField: 'time',
              yField: 'requests',
              seriesField: 'category',
              color: ['#5B8FF9', '#F46649', '#30BF78', '#FF9845'],
              theme: isDarkMode ? 'dark' : 'default',
          };
          return (
              <Suspense fallback={<div>Loading chart...</div>}>
                  <Column {...config} />
              </Suspense>
          );
      };

      export default EvolutionStacked;
      ```

### Tarea 7: Frontend — nuevo componente block_rate_chart.tsx

**Archivos:**
- Crear: `frontend/src/components/charts/block_rate_chart.tsx`

- [ ] **Paso 1:** Crear line chart para block rate:
      ```typescript
      import React, { Suspense } from 'react';
      import { Line } from '@ant-design/charts';

      interface BlockRateChartProps {
          data: Array<{ time: string; rate: number }>;
          isDarkMode: boolean;
      }

      const BlockRateChart: React.FC<BlockRateChartProps> = ({ data, isDarkMode }) => {
          const config = {
              data,
              xField: 'time',
              yField: 'rate',
              smooth: true,
              theme: isDarkMode ? 'dark' : 'default',
              yAxis: { max: 100, suffix: '%' },
          };
          return (
              <Suspense fallback={<div>Loading chart...</div>}>
                  <Line {...config} />
              </Suspense>
          );
      };

      export default BlockRateChart;
      ```

### Tarea 8: Frontend — nuevo componente top_methods.tsx

**Archivos:**
- Crear: `frontend/src/components/charts/top_methods.tsx`

- [ ] **Paso 1:** Crear pie o column chart para métodos HTTP:
      ```typescript
      import React, { Suspense } from 'react';
      import { Pie } from '@ant-design/charts';

      interface TopMethodsProps {
          data: Array<{ name: string; value: number }>;
          isDarkMode: boolean;
      }

      const TopMethods: React.FC<TopMethodsProps> = ({ data, isDarkMode }) => {
          const config = {
              data,
              angleField: 'value',
              colorField: 'name',
              label: { type: 'outer', content: '{name} ({percentage})' },
              theme: isDarkMode ? 'dark' : 'default',
          };
          return (
              <Suspense fallback={<div>Loading chart...</div>}>
                  <Pie {...config} />
              </Suspense>
          );
      };

      export default TopMethods;
      ```

### Tarea 9: Frontend — nuevo componente top_paths.tsx

**Archivos:**
- Crear: `frontend/src/components/charts/top_paths.tsx`

- [ ] **Paso 1:** Crear horizontal bar chart para paths:
      ```typescript
      import React, { Suspense } from 'react';
      import { Bar } from '@ant-design/charts';

      interface TopPathsProps {
          data: Array<{ name: string; value: number }>;
          isDarkMode: boolean;
      }

      const TopPaths: React.FC<TopPathsProps> = ({ data, isDarkMode }) => {
          const config = {
              data,
              xField: 'value',
              yField: 'name',
              seriesField: 'name',
              theme: isDarkMode ? 'dark' : 'default',
              label: { position: 'right' },
          };
          return (
              <Suspense fallback={<div>Loading chart...</div>}>
                  <Bar {...config} />
              </Suspense>
          );
      };

      export default TopPaths;
      ```

### Tarea 10: Frontend — nuevo componente evolution_by_method.tsx

**Archivos:**
- Crear: `frontend/src/components/charts/evolution_by_method.tsx`

- [ ] **Paso 1:** Crear multi-line chart para evolución por método:
      ```typescript
      import React, { Suspense } from 'react';
      import { Line } from '@ant-design/charts';

      interface EvolutionByMethodProps {
          data: Array<{ category: string; time: string; requests: number }>;
          isDarkMode: boolean;
      }

      const EvolutionByMethod: React.FC<EvolutionByMethodProps> = ({ data, isDarkMode }) => {
          const config = {
              data,
              xField: 'time',
              yField: 'requests',
              seriesField: 'category',
              smooth: true,
              theme: isDarkMode ? 'dark' : 'default',
              legend: { position: 'top' },
          };
          return (
              <Suspense fallback={<div>Loading chart...</div>}>
                  <Line {...config} />
              </Suspense>
          );
      };

      export default EvolutionByMethod;
      ```

### Tarea 11: Frontend — refactorizar charts_page.tsx

**Archivos:**
- Modificar: `frontend/src/pages/charts/charts_page.tsx`

- [ ] **Paso 1:** Añadir imports para los nuevos componentes y Tabs:
      ```typescript
      import { Tabs } from 'antd';
      import SummaryCards from '@/components/charts/summary_cards';
      import EvolutionStacked from '@/components/charts/evolution_stacked';
      import BlockRateChart from '@/components/charts/block_rate_chart';
      import TopMethods from '@/components/charts/top_methods';
      import TopPaths from '@/components/charts/top_paths';
      import EvolutionByMethod from '@/components/charts/evolution_by_method';
      ```

- [ ] **Paso 2:** Añadir nuevos campos al estado del componente:
      ```typescript
      interface State {
          // ... existing state fields ...
          topMethods: Array<{ name: string; value: number }>;
          topPaths: Array<{ name: string; value: number }>;
          evolutionByMethod: Array<{ category: string; time: string; requests: number }>;
          blockRateData: Array<{ time: string; rate: number }>;
      }
      ```

- [ ] **Paso 3:** En `componentDidMount`, añadir fetch a los nuevos endpoints usando `Promise.all()`:
      ```typescript
      const [evolutionRes, countriesRes, rulesRes, methodsRes, pathsRes] = await Promise.all([
          loadData('stats/evolution', new Map([['unit', unit], ['last', last.toString()]])),
          loadData('stats/top_countries'),
          loadData('stats/top_rules'),
          loadData('stats/top_methods'),
          loadData('stats/top_paths'),
      ]);
      ```

- [ ] **Paso 4:** Procesar `methodsRes` y `pathsRes` al mismo formato que los otros charts:
      ```typescript
      const topMethods = methodsRes.map(([name, value]: [string, number, number]) => ({
          name, value: value as number,
      }));
      const topPaths = pathsRes.map(([name, value]: [string, number, number]) => ({
          name, value: value as number,
      }));
      ```

- [ ] **Paso 5:** Calcular `blockRateData` a partir de los datos de evolución existentes:
      ```typescript
      const blockRateData = evolutionRes.map((bucket: any) => ({
          time: bucket.time,
          rate: bucket.blocked + bucket.allowed > 0
              ? (bucket.blocked / (bucket.blocked + bucket.allowed)) * 100
              : 0,
      }));
      ```

- [ ] **Paso 6:** Calcular `evolutionByMethod` — si no hay datos por método desde backend, usar los datos de evolución existentes con categoría "all":
      ```typescript
      const evolutionByMethod = evolutionRes.map((bucket: any) => ({
          category: 'all',
          time: bucket.time,
          requests: bucket.blocked + bucket.allowed,
      }));
      ```
      *(Nota: si en el futuro el backend devuelve evolución segmentada por método, se reemplaza este mapeo)*

- [ ] **Paso 7:** Calcular total/allowed/blocked para SummaryCards:
      ```typescript
      const total = evolutionRes.reduce((sum: number, b: any) => sum + b.blocked + b.allowed, 0);
      const allowed = evolutionRes.reduce((sum: number, b: any) => sum + b.allowed, 0);
      const blocked = evolutionRes.reduce((sum: number, b: any) => sum + b.blocked, 0);
      ```

- [ ] **Paso 8:** Preparar datos para `evolution_stacked`:
      ```typescript
      const evolutionStackedData = evolutionRes.flatMap((bucket: any) => [
          { category: 'Allowed', time: bucket.time, requests: bucket.allowed },
          { category: 'Blocked', time: bucket.time, requests: bucket.blocked },
      ]);
      ```

- [ ] **Paso 9:** Reemplazar el render existente con Tabs:
      ```tsx
      <ConfigProvider theme={...}>
          <Tabs defaultActiveKey="evolution">
              <Tabs.TabPane tab="Evolution" key="evolution">
                  <SummaryCards total={total} allowed={allowed} blocked={blocked} />
                  <Row gutter={16}>
                      <Col span={24}>
                          <Card title="Request Evolution (Stacked)">
                              <EvolutionStacked data={evolutionStackedData} isDarkMode={isDarkMode} />
                          </Card>
                      </Col>
                  </Row>
                  <Row gutter={16} style={{ marginTop: 16 }}>
                      <Col span={12}>
                          <Card title="Block Rate Over Time">
                              <BlockRateChart data={blockRateData} isDarkMode={isDarkMode} />
                          </Card>
                      </Col>
                      <Col span={12}>
                          <Card title="Evolution by Method">
                              <EvolutionByMethod data={evolutionByMethod} isDarkMode={isDarkMode} />
                          </Card>
                      </Col>
                  </Row>
              </Tabs.TabPane>
              <Tabs.TabPane tab="Rankings" key="rankings">
                  <Row gutter={16}>
                      <Col span={12}>
                          <Card title="Top Countries">
                              <TopCountriesChart data={topCountries} isDarkMode={isDarkMode} />
                          </Card>
                      </Col>
                      <Col span={12}>
                          <Card title="Top Rules">
                              <TopRulesChart data={topRules} isDarkMode={isDarkMode} />
                          </Card>
                      </Col>
                  </Row>
                  <Row gutter={16} style={{ marginTop: 16 }}>
                      <Col span={12}>
                          <Card title="Top Methods">
                              <TopMethods data={topMethods} isDarkMode={isDarkMode} />
                          </Card>
                      </Col>
                      <Col span={12}>
                          <Card title="Top Paths">
                              <TopPaths data={topPaths} isDarkMode={isDarkMode} />
                          </Card>
                      </Col>
                  </Row>
              </Tabs.TabPane>
          </Tabs>
      </ConfigProvider>
      ```

- [ ] **Paso 10:** Asegurar que `componentDidUpdate` tiene early return para cambios irrelevantes:
      ```typescript
      componentDidUpdate = async (prevProps, prevState) => {
          if (prevState.loading !== this.state.loading || prevState.items !== this.state.items) {
              return;
          }
          // ... resto de lógica ...
      }
      ```

### Tarea 12: Verificar reglas de concurrencia

**Archivos:**
- Revisar: `backend/src/models/stats.rs`

- [ ] **Paso 1:** Verificar que todos los `MutexGuard` se liberan antes de cualquier `.await` en stats.rs y en los callers (shuul.rs, report.rs).

- [ ] **Paso 2:** Verificar que el orden de locks se mantiene: rules → rate_limiter → ban_manager. StatsCollector no tiene dependencias cruzadas con estos locks, pero debe confirmarse que no se adquieren en orden inverso.