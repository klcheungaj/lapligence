// IEEE 1800-2009 4.4.2.4, 13.3.2, and 13.5: an NBA updates the static output
// formal after copy-out; the following invocation copies that retained value.
module tb;
    logic [7:0] actual;

    task stage_value(input logic [7:0] next_value,
                     output logic [7:0] staged_value);
        staged_value <= next_value;
    endtask

    initial begin
        stage_value(8'h11, actual);
        #1;
        stage_value(8'h22, actual);
        if (actual !== 8'h11) begin
            $display("FAIL static_task_nba second_copy");
            $finish;
        end
        #1;
        stage_value(8'h33, actual);
        if (actual !== 8'h22) begin
            $display("FAIL static_task_nba third_copy");
            $finish;
        end

        $display("PASS static_task_nba");
        $finish;
    end
endmodule
