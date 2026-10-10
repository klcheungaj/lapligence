// SIM-025 A01: $strobe and $monitor in static tasks and functions. The
// automatic-variable restriction of SV 13.3.2 covers only variables that die
// with the activation; static formals and locals, and module variables, are
// reported at the end of the slot like any other argument.
module tb;
  reg [3:0] a = 1;
  integer sv;

  task static t(input [3:0] v);
    reg [3:0] loc;
    begin
      loc = v + 4'd1;
      $strobe("task a=%0d v=%0d loc=%0d", a, v, loc);
      #1;
      loc = loc + 4'd1;
      $monitor("task monitor a=%0d loc=%0d", a, loc);
    end
  endtask

  function static void f(input [3:0] v);
    $strobe("func v=%0d sv=%0d", v, sv);
  endfunction

  function static void g();
    $fstrobe(1, "func fstrobe a=%0d", a);
  endfunction

  initial begin
    sv = 7;
    t(4'd7);
    f(4'd3);
    g();
    sv = 8;
    #1 a = 2;
    #1 $finish(0);
  end
endmodule
