import { Client } from '@elastic/elasticsearch'
import { useEffect, useRef, useState, useTransition } from 'react'
import { z } from 'zod'

/**
 * The result of a search against the ElasticSearch index, unmodified from the
 * `_source` in the response.
 */
const ElasticDocument = z.object({
    interview: z.string(),
    person: z.string(),
    statement: z.string(),
    text: z.string(),
    timestamp: z.object({
        gte: z.number(),
        lte: z.number(),
    }),
    highlight: z.object({
        text: z.array(z.string()),
    }),
}).readonly()
type ElasticDocument = z.infer<typeof ElasticDocument>

/**
 * The result of a search against the ElasticSearch index, parsed and validated
 * based on the `_source` in the response.
 */
const TranscriptHit = z.object({
    person: z.string(),
    interview: z.string(),
    statement: z.string(),
    text: z.string(),
    startTime: z.number(),
    endTime: z.number(),
    highlight: z.array(z.string()),
}).readonly()
type TranscriptHit = z.infer<typeof TranscriptHit>

function parseElasticDocument (hit: ElasticDocument): TranscriptHit {
}

export function useElasticTranscript (query: string) {
    const [hits, setHits] = useState<ElasticResult[]>([])
    const [error, setError] = useState<Error | null>(null)
    const [loading, setLoading] = useState(false)

    const client = useRef(
        new Client({
            node: 'https://elastic.auohp.here',
            tls: {
                rejectUnauthorized: false,
            },
        }),
    )

    useEffect(() => {
        if (
            !query ||
            typeof query !== 'string' ||
            query.length < 3 ||
            !client.current
        ) {
            return
        }

        const esClient = client.current

        async function search () {
            setLoading(true)

            try {
                const res = await esClient.search({
                    index: 'transcripts',
                    highlight: {
                        fields: {
                            text: {},
                        },
                        pre_tags: '{{',
                        post_tags: '}}',
                    },
                    query: {
                        match: {
                            text: query,
                        },
                    },
                })

                console.log(res.hits.hits[0]._source)
            } catch (e) {
                setError(e)
            } finally {
                setLoading(false)
            }
        }

        void search()
    }, [query])

    return {
        hits,
        error,
        loading,
    }
}

/* r = await client.search({
    index: 'transcripts',
    highlight: {
        fields: {
            text: {}
        },
        pre_tags: "{",
        post_tags: "}"
    },
    query: {
        match: {
            text: "church o'connor"
        }
    }
}) */
