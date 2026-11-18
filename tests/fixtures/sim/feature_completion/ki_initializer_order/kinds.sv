// Static declaration initializers of every kind, interleaved in one module, one
// package and one class: each initializer appends its name to its scope's log
// when it runs, so the logs show the order within each scope. Expected values
// are derived in readme.md.
package pkg;
  string log;
  function automatic int note(string s);
    log = {log, " ", s};
    return 1;
  endfunction
  function automatic string text(string s);
    log = {log, " ", s};
    return s;
  endfunction
  class item;
    function new(string s);
      log = {log, " ", s};
    endfunction
  endclass
  typedef struct { int a; int b; } pair_t;
  int scalar1 = note("scalar1");
  int pattern[2] = '{note("pattern"), 2};
  item handle = new("handle");
  string str = text("string");
  int queue[$] = '{note("queue"), 3};
  pair_t record = '{a: note("record"), b: 4};
  int dynamic[] = '{note("dynamic")};
  int assoc[string] = '{"k": note("assoc")};
  int scalar2 = note("scalar2");
endpackage

typedef class citem;
class holder;
  static string log;
  static function int note(string s);
    log = {log, " ", s};
    return 1;
  endfunction
  static function string text(string s);
    log = {log, " ", s};
    return s;
  endfunction
  static int scalar1 = note("scalar1");
  static int pattern[2] = '{note("pattern"), 2};
  static citem handle = new("handle");
  static string str = text("string");
  static int queue[$] = '{note("queue")};
  static pkg::pair_t record = '{a: note("record"), b: 1};
  static int dynamic[] = '{note("dynamic")};
  static int assoc[int] = '{7: note("assoc")};
  static int scalar2 = note("scalar2");
endclass
class citem;
  function new(string s);
    holder::log = {holder::log, " ", s};
  endfunction
endclass

module tb;
  string log;
  function automatic int note(string s);
    log = {log, " ", s};
    return 1;
  endfunction
  function automatic string text(string s);
    log = {log, " ", s};
    return s;
  endfunction
  class titem;
    function new(string s);
      log = {log, " ", s};
    endfunction
  endclass
  int scalar1 = note("scalar1");
  int pattern[2] = '{note("pattern"), 2};
  titem handle = new("handle");
  string str = text("string");
  int queue[$] = '{note("queue"), 3};
  pkg::pair_t record = '{a: note("record"), b: 4};
  int dynamic[] = '{note("dynamic")};
  int assoc[int] = '{5: note("assoc")};
  int scalar2 = note("scalar2");
  initial begin
    $display("package:%s", pkg::log);
    $display("class:%s", holder::log);
    $display("module:%s", log);
    $display("values %0d %0d %0d %0d %0d %0d %0d", scalar1 + scalar2, pattern[0], queue[0],
             record.a, dynamic[0], assoc[5], pkg::assoc["k"] + holder::assoc[7]);
    $finish;
  end
endmodule
