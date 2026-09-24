// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/record_auto_ref.sv
// IEEE 1800-2009 §§6.21, 7.4.2, 10.9.1-10.9.2, and 13.5.5: an automatic
// fixed array of records may be initialized and a selected record passed by ref.
module tb;
    typedef struct {
        logic [7:0] key;
        logic [7:0] payload;
    } record_t;
    typedef record_t records_t [0:1];

    task automatic set_key(ref record_t selected, input logic [7:0] next_key);
        selected.key = next_key;
    endtask

    task automatic check_activation(input logic [7:0] next_key);
        automatic records_t rows = '{
            record_t: '{key: 8'h22, payload: 8'hb2},
            0: '{key: 8'h11, payload: 8'ha1}
        };

        if (rows[0].key !== 8'h11 || rows[0].payload !== 8'ha1 ||
            rows[1].key !== 8'h22 || rows[1].payload !== 8'hb2)
            $fatal(1, "automatic record array initializer");

        set_key(rows[1], next_key);
        if (rows[0].key !== 8'h11 || rows[0].payload !== 8'ha1 ||
            rows[1].key !== next_key || rows[1].payload !== 8'hb2)
            $fatal(1, "ref selected record array element");

        $display("call=%h row0=%h/%h row1=%h/%h",
                 next_key, rows[0].key, rows[0].payload, rows[1].key, rows[1].payload);
    endtask

    initial begin
        check_activation(8'hc5);
        check_activation(8'he6);
        $finish(0);
    end
endmodule
