import neo4j, { type Driver } from 'neo4j-driver'
import { useEffect, useState } from 'react'

export function useNeo4j(url: string, username: string, password: string): Driver | null {
    const [driver, setDriver] = useState<Driver | null>(null)

    useEffect(() => {
        if (driver) {
            return
        }

        async function connect() {
            const driver = neo4j.driver(
                url,
                neo4j.auth.basic(username, password),
                {
                    disableLosslessIntegers: true,
                },
            )
            setDriver(driver)
            await driver.getServerInfo()
        }

        void connect()

        return () => {
            if (driver) {
                void driver.close()
            }
        }
    }, [ driver, password, url, username])

    return driver
}
