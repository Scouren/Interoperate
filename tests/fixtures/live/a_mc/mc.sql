-- A hand-written 3-sample Monte Carlo history (tests/maestro_results.rs): only the tables and
-- columns results::convert reads, by position; every value made up. Build with
--   sqlite3 MonteCarlo.0.rdb < mc.sql
PRAGMA page_size = 1024;
CREATE TABLE test(testID INTEGER PRIMARY KEY, name TEXT);
CREATE TABLE corner(cornerID INTEGER PRIMARY KEY, name TEXT);
-- Design points 4..6, so the sample can only come from testStatus's eighth column (1..3).
CREATE TABLE point(pointID INTEGER PRIMARY KEY, designPoint INTEGER, merit FLOAT, cornerID INTEGER);
CREATE TABLE error(errorID INTEGER PRIMARY KEY, message TEXT);
CREATE TABLE parameter(parameterID INTEGER PRIMARY KEY, name TEXT, c2, c3, c4, c5, c6, c7, c8, type TEXT);
CREATE TABLE testDefaultParameterValue(parameterID INTEGER, testID INTEGER, value);
CREATE TABLE parameterValue(pointID INTEGER, parameterID INTEGER, value);
CREATE TABLE result(resultID INTEGER PRIMARY KEY, name TEXT, testID INTEGER, flag TEXT);
CREATE TABLE spec(specID INTEGER PRIMARY KEY, resultID INTEGER);
CREATE TABLE specValue(pointID INTEGER, specID INTEGER, c2, c3, c4, c5, status INTEGER);
CREATE TABLE resultValue(pointID INTEGER, resultID INTEGER, value, errorID INTEGER);
CREATE TABLE testStatus(pointID INTEGER, testID INTEGER, c2, c3, c4, c5, errorID INTEGER, sample INTEGER);
INSERT INTO test VALUES (1, 'TM');
INSERT INTO corner VALUES (1, '');
INSERT INTO point VALUES (1, 4, 0.0, 1), (2, 5, 0.0, 1), (3, 6, 0.0, 1);
INSERT INTO error VALUES (1, 'done');
INSERT INTO parameter VALUES
  (1, 'rv', NULL, NULL, 1, NULL, NULL, NULL, NULL, 'desVar'),
  (2, 'mc::seed', NULL, NULL, 1, NULL, NULL, NULL, NULL, 'statistical');
INSERT INTO testDefaultParameterValue VALUES (1, 1, '');
INSERT INTO parameterValue VALUES (1, 1, '2k'), (2, 1, '2k'), (3, 1, '2k');
INSERT INTO result VALUES (1, 'vout', 1, '0');
INSERT INTO resultValue VALUES (1, 1, 0.48, NULL), (2, 1, 0.46, NULL), (3, 1, 0.5, NULL);
INSERT INTO testStatus VALUES
  (1, 1, 1, 0, 1, 3, 1, 1), (2, 1, 1, 1, 2, 3, 1, 2), (3, 1, 1, 2, 3, 3, 1, 3);
