// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_06/resolve_once.sv
// IEEE 1800-2009 11.4.1, 11.4.2 and 9.4.2: a selected lvalue whose selector
// has a side effect evaluates that selector exactly once. Statement targets
// (bit-select, indexed part-select, array element) and a ref actual each
// consume one increment.
module tb;
    logic [7:0] packed_value;
    logic [7:0] mem [0:3];
    integer i;
    integer calls;

    function automatic logic [7:0] f;
        begin
            calls = calls + 1;
            f = 8'hA5;
        end
    endfunction

    task automatic put(ref logic [7:0] dest, input logic [7:0] value);
        dest = value;
    endtask

    initial begin
        packed_value = 8'h00;
        i = 3;
        packed_value[i++] = 1'b1;
        $display("bit=%h bit_index=%0d", packed_value, i);

        packed_value = 8'h00;
        i = 2;
        packed_value[i++ +: 2] = 2'b11;
        $display("part=%h part_index=%0d", packed_value, i);

        i = 2;
        calls = 0;
        mem[i++] = f();
        put(mem[i++], f());
        $display("array=%h ref=%h index=%0d calls=%0d", mem[2], mem[3], i, calls);
        $finish(0);
    end
endmodule
