// `%t` in a string formatter reads its time value in the calling scope's
// time unit, as `$display` does (21.3.3, 20.4.2).
package p;
  timeunit 1ns;
  timeprecision 1ps;
  class c;
    static function string stamp();
      string s;
      $swrite(s, "%0t", $time);
      return {s, " | ", $sformatf("%0t", $time)};
    endfunction
  endclass
endpackage

module tb;
  timeunit 1us;
  timeprecision 1ps;
  import p::*;
  string s, f;
  initial begin
    #1;
    $display("default: display=%0t sformatf=%s pkg=%s", $time, $sformatf("%0t", $time), c::stamp());
    $timeformat(-9, 0, " ns", 5);
    #0.5;
    $swrite(s, "%t", $realtime);
    $sformat(f, "%0t", $realtime);
    $display("ns: display=%0t swrite=%s sformat=%s pkg=%s", $realtime, s, f, c::stamp());
    f = "%0t";
    $display("dynamic: %s", $sformatf(f, $realtime));
    $finish;
  end
endmodule
