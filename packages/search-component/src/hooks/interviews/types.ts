export type Propertized<T> = { properties: T, labels: string[] }

export interface Person {
  name: string
  uid: string
}

export interface Statement {
  text: string
}

export interface Interview {
  number: number
  uid: string
}

export interface Documentary {
  date: string
  title: string
  uid: string
  slug: string
}

export interface Video {
  url: string
  uid: string
}

export interface Broadsheet {
  title: string
  uid: string
}

export interface Asset {
  url: string
}

export interface Neo4jResult {
  person?: Propertized<Person>
  meta: Propertized<SaysEdge>
  statement: Propertized<Statement>
  artefact: Propertized<Documentary> | Propertized<Interview> | Propertized<Broadsheet>
  asset?: Propertized<Asset>
}

export interface SaysEdge {
  startTimestamp: string
  endTimestamp: string
  startTime: number
  endTime: number
}

export function isDocumentary(artefact: Propertized<Documentary> | Propertized<Interview> | Propertized<Broadsheet>): artefact is Propertized<Documentary> {
  return Array.isArray(artefact.labels) && artefact.labels.includes('Documentary')
}

export function isInterview(artefact: Propertized<Documentary> | Propertized<Interview> | Propertized<Broadsheet>): artefact is Propertized<Interview> {
  return Array.isArray(artefact.labels) && artefact.labels.includes('Interview')
}

export function isBroadsheet(artefact: Propertized<Documentary> | Propertized<Interview> | Propertized<Broadsheet>): artefact is Propertized<Broadsheet> {
  return Array.isArray(artefact.labels) && artefact.labels.includes('Broadsheet')
}
