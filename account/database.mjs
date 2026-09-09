// Small D1-compatible SQLite adapter for self-hosting and real SQL tests.
import { DatabaseSync } from 'node:sqlite';
export class Database {
  constructor(path=':memory:') {
    this.sqlite=new DatabaseSync(path);
    this.sqlite.exec('PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;');
  }
  prepare(sql) {
    const database=this;
    return new class Statement {
      constructor(values=[]) {this.values=values;}
      bind(...values) {return new Statement(values);}
      async first() {return database.sqlite.prepare(sql).get(...this.values)||null;}
      async all() {return {results:database.sqlite.prepare(sql).all(...this.values),success:true};}
      runSync() {const r=database.sqlite.prepare(sql).run(...this.values);return {success:true,meta:{changes:Number(r.changes)}};}
      async run() {return this.runSync();}
    }();
  }
  async batch(statements) {
    this.sqlite.exec('SAVEPOINT batch');
    try {const result=[];for(const statement of statements)result.push(statement.runSync());this.sqlite.exec('RELEASE batch');return result;}
    catch(error) {this.sqlite.exec('ROLLBACK TO batch; RELEASE batch');throw error;}
  }
  close() {this.sqlite.close();}
}
