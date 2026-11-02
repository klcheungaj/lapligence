// Decision A19-D75: the origin and statement of a frame. See
// docs/lrm_decisions.md for the decision and the expected output.
//
// IEEE 1800-2009 37.39 (SystemVerilog-1800-2009.txt L60131-L60157): the
// frame diagram draws vpiOrigin from a frame to a "scope", "task call",
// "func call", "method task call" or "method func call", and the stmt
// relation. Detail 1 (L60160): "Frames correspond to the set of automatic
// variables declared in a given task or function." Detail 4 (L60163-L60165):
// "The frame to stmt transition shall return the currently active statement
// within the frame." 37.38 (method func call, method task call): a call of a
// class method, whose vpiFunction or vpiTask is the method.
//
// Text silent, industry-standard behaviour followed: the origin of a frame
// is the source call that activated it (a recursive call for an inner
// frame), and NULL for a frame no thread activated: a function a continuous
// assignment evaluates. The active statement of a frame that called another
// is the statement holding that call (the call itself for a task call
// statement). A call of a static method through its class (`C::s(...)`) is
// a method call.
module tb;
  int src, g;
  wire [31:0] w;
  assign w = cf(src);
  function automatic int cf(int v);
    $d75(1);
    return v * 2;
  endfunction
  function automatic int fact(int n);
    if (n <= 1) begin
      $d75(2);
      return 1;
    end
    return n * fact(n - 1);
  endfunction
  function automatic void vf(int x);
    $d75(3);
  endfunction
  task automatic rt(int n);
    if (n > 0) rt(n - 1);
    else $d75(4);
  endtask
  class C;
    int m;
    function int fm(int k);
      $d75(5);
      return k + m;
    endfunction
    task tm();
      $d75(6);
    endtask
    static function int sf(int k);
      $d75(7);
      return k;
    endfunction
  endclass
  initial begin
    C c;
    src = 3;
    #1;
    g = fact(2);
    vf(4);
    rt(1);
    c = new;
    if (c.fm(1) > 0) c.tm();
    g = C::sf(8);
    $display("g=%0d w=%0d", g, w);
    $finish;
  end
endmodule
