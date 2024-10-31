import neo4j, { type Driver } from 'neo4j-driver'
import { useEffect, useState } from 'react'


const {
  VITE_NEO4J_PASSWORD: NEO4J_PASSWORD = 'auohpauohp',
  VITE_NEO4J_URI: NEO4J_URI = 'bolt+s://bolt.auohp.here:443',
  VITE_NEO4J_USERNAME: NEO4J_USERNAME = 'neo4j',
} = import.meta.env


export function useNeo4j(
  uri: string = NEO4J_URI,
  username: string = NEO4J_USERNAME,
  password: string = NEO4J_PASSWORD,
): Driver | null {
  const [driver, setDriver] = useState<Driver | null>(null)

  useEffect(() => {
    if (driver) {
      return
    }

    async function connect() {
      const driver = neo4j.driver(
        uri,
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
  }, [driver, password, uri, username])

  return driver
}
