// llg-test-fixture: tests/fixtures/sim/partial_features/display_monitor_typed.sv
module tb;
    real amount;
    string text;

    initial begin
        amount = 1.0;
        text = "a";
        $monitor("monitor=%s %.1f %m", text, amount);
        #1 amount = 2.0;
        #1 $finish;
    end
endmodule
