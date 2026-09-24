// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/written_sources_call_arguments.sv
// IEEE 1800-2009 §§6.19, 9.2.2.2, 10.3 and 13.4.1.
module tb;
    logic clk = 1'b0;
    logic [7:0] nba_source = 8'h11;
    logic [7:0] nba_next = 8'h5a;
    logic [7:0] nba_pre_sample = 8'h00;
    logic [7:0] nba_call_sample = 8'h00;
    logic [7:0] nba_neighbor = 8'hc3;

    logic [7:0] net_seed = 8'h11;
    wire [7:0] net_source;
    logic [7:0] net_call_sample = 8'h00;
    logic [7:0] net_neighbor = 8'hd4;

    logic [7:0] variable_seed = 8'h11;
    logic [7:0] variable_source;
    logic [7:0] variable_call_sample = 8'h00;
    logic [7:0] variable_neighbor = 8'he5;

    assign net_source = net_seed;
    assign variable_source = variable_seed;

    task automatic capture_nba(input logic [7:0] value);
        nba_call_sample = value;
    endtask

    task automatic capture_net(input logic [7:0] value);
        net_call_sample = value;
    endtask

    task automatic capture_variable(input logic [7:0] value);
        variable_call_sample = value;
    endtask

    always_ff @(posedge clk) begin
        nba_pre_sample <= nba_source;
        nba_source <= nba_next;
    end

    initial begin
        #1;
        if (nba_source !== 8'h11 ||
            nba_pre_sample !== 8'h00 ||
            nba_call_sample !== 8'h00 ||
            nba_neighbor !== 8'hc3)
            $fatal(1, "NBA source baseline mismatch");

        clk = 1'b1;
        #1;
        capture_nba(nba_source);
        if (nba_source !== 8'h5a ||
            nba_pre_sample !== 8'h11 ||
            nba_call_sample !== 8'h5a ||
            nba_neighbor !== 8'hc3)
            $fatal(1, "task input did not receive the post-NBA source value");
        $display("nba_call=%02h/%02h", nba_source, nba_call_sample);

        capture_net(net_source);
        capture_variable(variable_source);
        if (net_source !== 8'h11 ||
            net_call_sample !== 8'h11 ||
            net_neighbor !== 8'hd4 ||
            variable_source !== 8'h11 ||
            variable_call_sample !== 8'h11 ||
            variable_neighbor !== 8'he5)
            $fatal(1, "continuous source baseline mismatch");

        net_seed = 8'h5a;
        variable_seed = 8'h5a;
        #1;
        if (net_source !== 8'h5a ||
            net_call_sample !== 8'h11 ||
            net_neighbor !== 8'hd4 ||
            variable_source !== 8'h5a ||
            variable_call_sample !== 8'h11 ||
            variable_neighbor !== 8'he5)
            $fatal(1, "continuous source phase update mismatch");

        capture_net(net_source);
        capture_variable(variable_source);
        if (net_source !== 8'h5a ||
            net_call_sample !== 8'h5a ||
            net_neighbor !== 8'hd4 ||
            variable_source !== 8'h5a ||
            variable_call_sample !== 8'h5a ||
            variable_neighbor !== 8'he5)
            $fatal(1, "task inputs did not receive the settled continuous values");
        $display("continuous_call=%02h/%02h", net_call_sample, variable_call_sample);
        $finish(0);
    end
endmodule
