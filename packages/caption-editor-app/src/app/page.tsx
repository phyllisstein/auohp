export default async function Page() {
    async function listInterviews() {
        "use server";

        return fetch("http://127.0.0.1:3030/api/interviews")
            .then(async response => {
                if (!response.ok) {
                    console.error(response);
                    throw new Error("Failed to fetch interviews");
                }
                const json = await response.json();
                return json;
            })
            .catch(console.error);
    }

    const interviews = await listInterviews();

    if (!Array.isArray(interviews)) {
        return null;
    }

    return (
        <div>
            <ul>
                { interviews.map(interview => (
                    <li key={ interview.uid }>
                        <a href={ `/transcript/${ interview.number }` }>{ interview.date } – { interview.title }</a>
                    </li>
                )) }
            </ul>
        </div>
    );
}
