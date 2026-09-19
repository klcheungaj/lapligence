// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_12/overlap_concat_store.sv
// IEEE 1800-2009 10.4 and 11.4.14: a concatenation assignment and a streaming
// copy capture the source before any overlapping destination bit is written.
module tb;
    logic [7:0] value;
    logic [7:0] lanes [0:1];
    logic [15:0] word;

    initial begin
        value = 8'h0f;
        {value[3:0], value[7:4]} = value;
        $display("concat %h", value);

        value = 8'h96;
        {value[3:0], value[7:4]} = value;
        $display("concat_overlap %h", value);

        lanes[0] = 8'h11;
        lanes[1] = 8'h22;
        {>>{lanes[0], lanes[1]}} = {lanes[1], lanes[0]};
        $display("stream %h %h", lanes[0], lanes[1]);

        word = 16'ha55a;
        word[7:0] = word[15:8];
        $display("part %h", word);
        $finish(0);
    end
endmodule
