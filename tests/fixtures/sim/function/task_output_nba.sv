module tb;
    logic clk;
    logic [3:0] q;
    logic [3:0] d;

    task set_q(input logic [3:0] x, output logic [3:0] y);
        y <= x + 1;
    endtask

    always #5 clk = ~clk;

    always @(posedge clk) begin
        d <= 4'd7;
        set_q(d, q);
    end

    initial begin
        clk = 0;
        #26;
        if (q !== 4'd8) begin
            $display("FAIL task_output_nba persisted_copy got=%b", q);
            $finish;
        end
        #10;
        if (q !== 4'd8) begin
            $display("FAIL task_output_nba steady_copy got=%b", q);
            $finish;
        end
        $display("PASS task_output_nba");
        $finish;
    end
endmodule
