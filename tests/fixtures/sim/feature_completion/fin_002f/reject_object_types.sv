// Every marked line is refused and no other one is: virtual interfaces (25.9),
// mailboxes (15.4.9), $cast (6.24.2) and the coverage methods (19.8).
`timescale 1ns/1ns
interface ia #(int W = 8) (input logic clk);
  logic [W-1:0] d;
  modport m(input d);
  modport s(output d);
endinterface
interface ib(input logic clk);
  logic [7:0] d;
endinterface
module tb;
  class c_t;
    int v;
  endclass
  typedef struct {
    int a;
    string s;
  } st_t;
  typedef struct packed {logic [3:0] h, l;} ps_t;

  bit clk;
  ia narrow(clk);
  ia #(16) broad(clk);
  ib other(clk);
  ia many[2](clk);
  virtual ia v;
  virtual ia #(16) v16;
  virtual ia.m vm;
  virtual ia.s vs;
  virtual ib w;
  virtual ia.zz vz;  // wrong: SLANG-NotAModport
  c_t h;
  int x;
  mailbox #(int) mi = new;
  mailbox #(c_t) mc = new;
  mailbox #(string) ms = new;
  st_t st;
  ps_t ps;
  event ev;
  covergroup cg @(posedge clk);
    cp: coverpoint x {bins b[] = {[0:3]};}
  endgroup
  cg cgi = new;

  function void take(virtual ia formal);
  endfunction

  initial begin
    v = narrow;
    v16 = broad;
    vm = v;
    vm = narrow;
    v = many[1];
    v = other;  // wrong: SLANG-BadAssignment
    v = broad;  // wrong: SLANG-BadAssignment
    v = v16;  // wrong: SLANG-BadAssignment
    v = w;  // wrong: SLANG-BadAssignment
    v = vm;  // wrong: SLANG-BadAssignment
    vs = vm;  // wrong: SLANG-BadAssignment
    v = 3;  // wrong: SLANG-BadAssignment
    h = new;
    v = h;  // wrong: SLANG-BadAssignment
    take(w);  // wrong: SLANG-BadAssignment
    x = v + 1;  // wrong: SLANG-BadBinaryExpression
    x = v.nowhere;  // wrong: SLANG-CouldNotResolveHierarchicalPath
    vm.d = 3;  // wrong: SLANG-InputPortAssign
  end
  initial begin
    mi.put(3);
    mi.put("text");
    mi.put(h);  // wrong: SLANG-NoImplicitConversion
    mi.put(st);  // wrong: SLANG-NoImplicitConversion
    mi.put(1, 2);  // wrong: SLANG-TooManyArguments
    mc.put(h);
    mc.put(5);  // wrong: SLANG-BadAssignment
    mi.get(h);  // wrong: SLANG-RefTypeMismatch
    ms.get(x);  // wrong: SLANG-RefTypeMismatch
    mc.get(x);  // wrong: SLANG-RefTypeMismatch
    x = mi.num(1);  // wrong: SLANG-TooManyArguments
    mi = mc;  // wrong: SLANG-BadAssignment
    mi = h;  // wrong: SLANG-BadAssignment
  end
  initial begin
    void'($cast(x, 3));
    void'($cast(h, null));
    void'($cast(x));  // wrong: SLANG-TooFewArguments
    void'($cast(3, x));  // wrong: SLANG-ExpressionNotAssignable
  end
  initial begin
    cgi.sample();
    cgi.sample(3);  // wrong: SLANG-TooManyArguments
    cgi.set_inst_name(5);  // wrong: SLANG-NoImplicitConversion
    cgi.start(1);  // wrong: SLANG-TooManyArguments
    cgi.cp.sample();  // wrong: SLANG-UnknownMember
    cgi.cp.nowhere();  // wrong: SLANG-UnknownMember
    cgi.option.nowhere = 1;  // wrong: SLANG-UnknownMember
    x = cgi.get_inst_coverage(x, x, x);  // wrong: SLANG-TooManyArguments
    cgi = h;  // wrong: SLANG-BadAssignment
  end
endmodule
