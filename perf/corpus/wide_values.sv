`ifndef LLG_CORPUS_N
`define LLG_CORPUS_N 128
`endif

`ifndef LLG_CORPUS_WIDTH
`define LLG_CORPUS_WIDTH 4096
`endif

`ifndef LLG_CORPUS_ROUNDS
`define LLG_CORPUS_ROUNDS 2000
`endif

module wide_values #(
    parameter integer N = `LLG_CORPUS_N,
    parameter integer WIDTH = `LLG_CORPUS_WIDTH,
    parameter integer ROUNDS = `LLG_CORPUS_ROUNDS
);
    logic tick = 0;
    logic [N-1:0] done = '0;
    integer value_checksum = 0;

    task automatic exercise_wide_value(output logic worker_done,
                                       input integer id,
                                       ref logic source);
        logic [WIDTH-1:0] value;
        string label;
        integer round;

        value = '0;
        value[id % WIDTH] = 1'b1;
        label = $sformatf("worker-%0d-width-%0d", id, WIDTH);
        worker_done = 0;
        for (round = 0; round < ROUNDS; round = round + 1) begin
            @(posedge source);
            value = {value[WIDTH-2:0], value[WIDTH-1] ^ round[0]};
            label = {label, round[0] ? "1" : "0"};
            #0;
        end
        value_checksum = value_checksum + $countones(value) + label.len() + id;
        worker_done = ((^value) !== 1'bx) && (label.len() > 0);
    endtask

    genvar i;
    for (i = 0; i < N; i = i + 1) begin : workers
        initial exercise_wide_value(done[i], i, tick);
    end

    always #1 tick = ~tick;

    initial begin
        wait (&done);
        #1;
        $display("wide_values n=%0d width=%0d rounds=%0d done=%0d checksum=%0d",
                 N, WIDTH, ROUNDS, $countones(done), value_checksum);
        $finish(0);
    end
endmodule
