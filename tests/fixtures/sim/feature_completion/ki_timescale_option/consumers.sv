// No design element declares a time scale, so every one takes the default:
// 1ns/1ps, or the --timescale value. See readme.md for the expected lines.
package p;
  task automatic wait_unit;
    #1;
  endtask
endpackage

module sub;
  initial #2 $display("sub t=%0t T=%t time=%0d", $time, $time, $time);
endmodule

module tb;
  sub u();
  initial begin
    $printtimescale;
    $printtimescale(u);
    $display("tf0=%t|", 1);
    #1 $display("t=%0t time=%0d real=%f", $time, $time, $realtime);
    #1.6 $display("t=%0t time=%0d real=%f", $time, $time, $realtime);
    p::wait_unit();
    $display("pkg t=%0t real=%f", $realtime, $realtime);
    $timeformat(-9, 1, " ns", 12);
    $display("tf=%t|", $realtime);
    $finish;
  end
endmodule
