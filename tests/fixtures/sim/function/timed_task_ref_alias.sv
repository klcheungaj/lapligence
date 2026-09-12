// llg-test-fixture: tests/fixtures/sim/function/timed_task_ref_alias.sv
module tb;
    logic [7:0] value;

    task automatic delayed_increment(ref logic [7:0] target);
        #1;
        target = target + 1'b1;
    endtask

    initial begin
        value = 8'd41;
        delayed_increment(value);
        $display("ref value=%0d t=%0t", value, $time);
        $finish;
    end
endmodule
