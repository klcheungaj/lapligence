// Decision A19-D76: the automatic variables of a frame and their values.
// See docs/lrm_decisions.md for the decision and the expected output.
//
// IEEE 1800-2009 37.39 (SystemVerilog-1800-2009.txt L60131-L60157): the
// frame diagram draws vpiAutomatics from a frame to "variables", "named
// event" and "named event array". Detail 1 (L60160): "Frames correspond to
// the set of automatic variables declared in a given task or function."
// 13.3.1 (L18621-L18622): "All items declared inside automatic tasks are
// allocated dynamically for each invocation. All formal arguments and local
// variables are stored on the stack."; (L18627-L18628) "Specific local
// variables can be declared as automatic within a static task or as static
// within an automatic task."
//
// Text silent, industry-standard behaviour followed: a task or function
// frame lists, in this order, the function-name return variable, `this` of a
// non-static method, the formals of every direction in declaration order,
// then the automatic variables the body declares at its top level in
// declaration order; array variables (fixed-size, dynamic, queue) follow
// all the others, formals first. Static variables are not listed, and a
// static task or function lists none. Values are current wherever the frame
// is seen: the active frame, a calling frame, a suspended frame, after a
// write by vpi_put_value and through a `ref` formal.
//
// Decided by llg (the text is silent; another simulator differs): the
// automatics of a named or unnamed block, a loop and a fork branch inside
// the subprogram belong to frames of those blocks, which llg does not model
// (decision A19-D44), so they are not listed.
module tb;
  class C;
    int m;
    function new(int v);
      m = v;
    endfunction
    function string sm(int k);
      $d76(1);
      return "s";
    endfunction
  endclass
  int gq[$];
  int gr, res;
  string gs;
  task automatic t(input int a, output int o, inout int io, ref int r, input int qa[$],
                   input logic [69:0] wv);
    int loc = a + 1;
    static int st = 5;
    int la[2] = '{5, 6};
    string s = "hi";
    real rr = 1.5;
    C lc = new(7);
    int lq[$] = '{9};
    o = loc;
    io = io + 1;
    r = r + 2;
    begin : nb
      int inner = loc * 3;
      $d76(2);
      res = inner;
    end
    for (int i = 0; i < 1; i++) $d76(3);
    #1 $d76(4);
    st = st + 1;
  endtask
  function automatic int f(int x);
    int y = x + 1;
    $d76(5);
    return y;
  endfunction
  task ts(input int a);
    int s2;
    s2 = a;
    $d76(6);
  endtask
  initial begin
    C c;
    int o, io;
    c = new(1);
    gs = c.sm(2);
    gq.push_back(3);
    io = 10;
    gr = 20;
    t(3, o, io, gr, gq, 70'h3);
    $display("o=%0d io=%0d gr=%0d res=%0d", o, io, gr, res);
    res = f(1);
    $display("f=%0d", res);
    ts(4);
    $finish;
  end
endmodule
