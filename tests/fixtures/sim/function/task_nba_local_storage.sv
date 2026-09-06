module tb;
    logic [7:0] before_value;
    logic [7:0] after_value;

    task schedule_local(
        input logic [7:0] next_value,
        output logic [7:0] before_result,
        output logic [7:0] after_result
    );
        logic [7:0] stored;
        before_result = stored;
        stored <= next_value;
        #1 after_result = stored;
    endtask

    initial begin
        schedule_local(8'h5a, before_value, after_value);
        if (before_value !== 8'hxx || after_value !== 8'h5a) begin
            $display("FAIL task_nba_local_storage first_nba");
            $finish;
        end

        schedule_local(8'ha5, before_value, after_value);
        if (before_value !== 8'h5a || after_value !== 8'ha5) begin
            $display("FAIL task_nba_local_storage second_nba");
            $finish;
        end

        $display("PASS task_nba_local_storage");
        $finish;
    end
endmodule
