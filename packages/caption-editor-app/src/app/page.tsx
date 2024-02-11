import { Header } from './page-styles'

export default async function Page() {
    const res = await fetch('https://httpbin.org/base64/SGVsbG8gV29ybGQ=')
    const js = await res.text()

    return (
        <Header>
            You did the thing: { js }
        </Header>
    )
}
