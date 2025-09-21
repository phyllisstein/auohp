import { updateTranscriptStatement } from "app/neo4j";
import type { NextRequest } from "next/server";

export async function PUT(request: NextRequest) {
    try {
        const requestData = await request.json();
        const response = await updateTranscriptStatement(requestData);

        return Response.json(response, { status: 201 });
    } catch (err) {
        return Response.json({
            error: "Failed to update transcript",
            message: err.message,
        }, { status: 500 });
    }
}
