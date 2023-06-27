import { Storage } from '@google-cloud/storage'

const storage = new Storage()
const bucketName = 'auohp'

const LOCAL_ASSET == "$PWD/bin"

await storage.bucket(bucketName).upload('mascot.gif', bucketName)
