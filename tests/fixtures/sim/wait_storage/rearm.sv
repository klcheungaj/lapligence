module tb;
    logic clk = 0;
    logic [64:0] wide = 0;
    event tick;
    int edges = 0;
    int wide_changes = 0;
    int events = 0;
    int mixed = 0;
    int cancelled = 0;

    initial begin
        repeat (4) begin
            @(posedge clk);
            edges++;
        end
    end
    initial begin
        repeat (4) begin
            @(wide);
            wide_changes++;
        end
    end
    initial begin
        repeat (4) begin
            @(tick);
            events++;
        end
    end
    initial begin
        repeat (4) begin
            @(posedge clk or tick);
            mixed++;
        end
    end
    initial begin : parked
        @(posedge clk);
        cancelled++;
        @(tick);
        cancelled += 100;
    end
    initial begin
        #1 clk = 1;
        #1 disable parked;
        -> tick;
        wide[64] = 1;
        #1 clk = 0;
        #1 clk = 1;
        -> tick;
        wide[0] = 1;
        #1 clk = 0;
        #1 clk = 1;
        -> tick;
        wide[64] = 0;
        #1 clk = 0;
        #1 clk = 1;
        -> tick;
        wide[0] = 0;
        #1;
        $display("%0d %0d %0d %0d %0d", edges, wide_changes, events, mixed, cancelled);
        $finish(0);
    end
endmodule
