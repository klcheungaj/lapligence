// llg-test-fixture: tests/fixtures/sim/data_types_next/syn_015_runtime_streams.sv
// IEEE 1800-2009 §§11.4.14, 11.4.14.3, 11.4.14.4: runtime-sized streams are
// left-aligned in wider fixed targets, and every `with` range streams its
// elements in storage order like an array slice, for sources and targets.
module tb;
    logic [7:0] up [0:2];
    logic [7:0] down [2:0];
    logic [7:0] queue_value [$];
    logic [23:0] wide;
    logic [15:0] pair;
    integer base;

    task automatic check(input bit condition, input string label);
        if (!condition) $fatal(1, "SYN015 runtime %s", label);
    endtask

    initial begin
        up = '{8'h11, 8'h22, 8'h33};
        down = '{8'hcc, 8'hbb, 8'haa};
        queue_value = '{8'h44, 8'h55};

        wide = {>>8{queue_value}};
        check(wide === 24'h44_55_00, "queue source left-aligned");
        base = 0;
        wide = {>>8{up with [base +: 2]}};
        check(wide === 24'h11_22_00, "runtime with source left-aligned");
        base = 2;
        pair = {>>8{up with [base -: 2]}};
        check(pair === 16'h22_33, "ascending -: storage order");
        base = 0;
        pair = {>>8{down with [base +: 2]}};
        check(pair === 16'hbb_aa, "descending +: storage order");
        pair = {>>8{down with [0 +: 2]}};
        check(pair === 16'hbb_aa, "static descending +: storage order");
        base = 2;
        queue_value.push_back(8'h66);
        pair = {>>8{queue_value with [base -: 2]}};
        check(pair === 16'h55_66, "queue -: storage order");

        base = 2;
        up = '{8'h00, 8'h00, 8'h00};
        {>>8{up with [base -: 2]}} = 16'hc1_d2;
        check(up[0] === 8'h00 && up[1] === 8'hc1 && up[2] === 8'hd2,
              "ascending -: target order");
        base = 0;
        down = '{8'h00, 8'h00, 8'h00};
        {<<8{down with [base +: 2]}} = 16'h55_66;
        check(down[2] === 8'h00 && down[1] === 8'h66 && down[0] === 8'h55,
              "descending reversed target order");
        $display("SYN015_RUNTIME_STREAMS_PASS");
        $finish(0);
    end
endmodule
