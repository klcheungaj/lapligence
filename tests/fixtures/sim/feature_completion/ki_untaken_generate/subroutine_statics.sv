package util;
  int seen[string];
  function automatic int mark(string s);
    if (seen.exists(s)) seen[s]++;
    else seen[s] = 1;
    return 1;
  endfunction
endpackage
module tb;
  import util::*;
  if (0) begin : x
    int b = mark("tb.x.b");
    function int f();
      static int s = mark("tb.x.f.s");
      return s;
    endfunction
    task t();
      static int s = mark("tb.x.t.s");
      $display("tb.x.t %0d", s);
    endtask
    initial void'(mark("tb.x.initial"));
    always begin
      void'(mark("tb.x.always"));
      #5;
    end
    final $display("tb.x final");
  end else begin : y
    int b = mark("tb.y.b");
    function int f();
      static int s = mark("tb.y.f.s");
      return s;
    endfunction
    task t();
      static int s = mark("tb.y.t.s");
      $display("tb.y.t %0d", s);
    endtask
    initial void'(mark("tb.y.initial"));
    final $display("tb.y final");
  end
  for (genvar j = 0; j < 0; j++) begin : z
    int b = mark("tb.z.b");
    function int f();
      static int s = mark("tb.z.f.s");
      return s;
    endfunction
    initial void'(mark("tb.z.initial"));
  end
  case (2)
    1: begin : c1
      int b = mark("tb.c1.b");
      function int f();
        static int s = mark("tb.c1.f.s");
        return s;
      endfunction
      initial void'(mark("tb.c1.initial"));
    end
    default: begin : cd
      int b = mark("tb.cd.b");
      function int f();
        static int s = mark("tb.cd.f.s");
        return s;
      endfunction
    end
  endcase
  initial begin
    #1;
    foreach (seen[s]) $display("%s %0d", s, seen[s]);
    $display("y.b + y.f() + cd.b + cd.f() = %0d", y.b + y.f() + cd.b + cd.f());
    y.t();
    $finish;
  end
endmodule
