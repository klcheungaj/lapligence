// IEEE 1800-2009 6.16: delayed string updates capture owned bytes and do
// not retain a dangling expression or automatic frame after scheduling.
module tb;
  string value;
  string blocking_value;

  initial begin
    blocking_value = "before";
    blocking_value = #1 "after";
    if (blocking_value != "after") begin
      $display("FAIL blocking delayed string assignment");
      $finish;
    end

    value = "old";
    value <= #2 "new";
        #1;
        if (value != "old") begin
            $display("FAIL string_delayed_nba early");
            $finish;
        end
        #2;
        if (value != "new") begin
            $display("FAIL string_delayed_nba commit");
            $finish;
        end
        $display("PASS string_delayed_nba");
        $finish;
    end
endmodule
