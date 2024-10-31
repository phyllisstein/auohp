import { describe, it, expect } from 'vitest'

const SOME_STRING = 'some string'

describe('expect (best practice)', () => {
  describe('is so ugly', () => {
    it('should assert that a string is equal to another', () => {
      expect(SOME_STRING).toEqual('some string')
    })

    it('should assert that a string is not equal to another', () => {
      expect(SOME_STRING).not.toEqual('another string')
    })
  })
})


describe('chai (YOOPOO: You Only Overwrite Prototype of Object Once)', () => {
  describe('is so beautiful', () => {
    it('should assert that a string is equal to another', () => {
      SOME_STRING.should.equal('some string')
    })

    it('should assert that a string is not equal to another', () => {
      SOME_STRING.should.not.equal('another string')
    })
  })
})
