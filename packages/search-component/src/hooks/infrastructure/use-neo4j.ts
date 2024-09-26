import neo4j, {type Driver} from 'neo4j-driver'
import {useEffect, useState} from 'react'

const {
  NEO4J_URI = 'neo4j://localhost:7687',
  NEO4J_USERNAME = 'neo4j',
  NEO4J_PASSWORD = 'auohpauohp',
} = process.env
}

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
