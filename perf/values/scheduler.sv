// Value-backend workload: scheduler traffic with small value payloads. Many
// processes alternate edge waits, delays, level waits, named events, NBAs and
// short-lived fork/join_none children, so scheduling dominates value work.

`ifndef LLG_VB_PROCS
`define LLG_VB_PROCS 512
`endif

`ifndef LLG_VB_ROUNDS
`define LLG_VB_ROUNDS 400
`endif

module scheduler #(
    parameter int PROCS = `LLG_VB_PROCS,
    parameter int ROUNDS = `LLG_VB_ROUNDS
);
    logic clk = 1'b0;
    logic [31:0] slots [PROCS];
    logic [15:0] tokens [PROCS];
    logic [PROCS-1:0] done = '0;
    logic [7:0] phase = '0;
    int spawned = 0;
    event round_event;

    always #2 clk = ~clk;
    always @(posedge clk) begin
        phase <= phase + 8'd1;
        if (phase[1:0] == 2'b11) -> round_event;
    end

    task automatic worker(input int id);
        logic [31:0] local_value;
        logic [15:0] token;
        local_value = 32'(id) * 32'h9e37_79b9;
        token = 16'(id);
        for (int round = 0; round < ROUNDS; round++) begin
            case ((id + round) % 4)
                0: @(posedge clk);
                1: #(1 + (id % 3));
                2: wait (phase[0] == id[0]);
                default: @(round_event);
            endcase
            local_value = (local_value ^ (local_value << 5)) + 32'(round);
            slots[id] <= local_value;
            token = token + local_value[15:0];
            tokens[id] <= token;
            if (round % 64 == id % 64) begin
                fork
                    begin
                        #1 slots[(id + 1) % PROCS] <= slots[(id + 1) % PROCS] ^ local_value;
                        spawned++;
                    end
                join_none
            end
        end
        done[id] = 1'b1;
    endtask

    genvar i;
    for (i = 0; i < PROCS; i = i + 1) begin : workers
        initial worker(i);
    end

    initial begin
        logic [31:0] checksum;
        wait (&done);
        #4;
        checksum = '0;
        for (int i = 0; i < PROCS; i++) checksum = (checksum ^ slots[i]) + 32'(tokens[i]);
        $display("scheduler procs=%0d rounds=%0d spawned=%0d checksum=%h", PROCS, ROUNDS,
                 spawned, checksum);
        $finish(0);
    end
endmodule
