export type Propertized<T> = { labels: string[]; properties: T };

export interface Person {
    name: string;
    uid: string;
}

export interface Statement {
    text: string;
}

export interface Interview {
    number: number;
    uid: string;
}

export interface Documentary {
    date: string;
    slug: string;
    title: string;
    uid: string;
}

export interface Video {
    uid: string;
    url: string;
}

export interface Broadsheet {
    title: string;
    uid: string;
}

export interface Asset {
    url: string;
}

export interface Neo4jResult {
    artefact: Propertized<Documentary> | Propertized<Interview> | Propertized<Broadsheet>;
    asset?: Propertized<Asset>;
    meta: Propertized<SaysEdge>;
    person?: Propertized<Person>;
    statement: Propertized<Statement>;
}

export interface SaysEdge {
    endTime: number;
    endTimestamp: string;
    startTime: number;
    startTimestamp: string;
}

export function isDocumentary(artefact: Propertized<Documentary> | Propertized<Interview> | Propertized<Broadsheet>): artefact is Propertized<Documentary> {
    return Array.isArray(artefact.labels) && artefact.labels.includes("Documentary");
}

export function isInterview(artefact: Propertized<Documentary> | Propertized<Interview> | Propertized<Broadsheet>): artefact is Propertized<Interview> {
    return Array.isArray(artefact.labels) && artefact.labels.includes("Interview");
}

export function isBroadsheet(artefact: Propertized<Documentary> | Propertized<Interview> | Propertized<Broadsheet>): artefact is Propertized<Broadsheet> {
    return Array.isArray(artefact.labels) && artefact.labels.includes("Broadsheet");
}
