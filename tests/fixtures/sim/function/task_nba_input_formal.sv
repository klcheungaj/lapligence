module tb;
    logic [7:0] observed;

    task schedule_input(input logic [7:0] value, output logic [7:0] result);
        value <= value + 8'h01;
        #1 result = value;
    endtask

    initial begin
        schedule_input(8'h10, observed);
        if (observed !== 8'h11) begin
            $display("FAIL task_nba_input_formal first_nba");
            $finish;
        end

        schedule_input(8'h20, observed);
        if (observed !== 8'h21) begin
            $display("FAIL task_nba_input_formal second_nba");
            $finish;
        end

        $display("PASS task_nba_input_formal");
        $finish;
    end
endmodule
