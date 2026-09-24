// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/generate_port.sv
// IEEE 1800-2009 §§7.2, 7.4, 13.4.1, 23.3.3, and 27.4: an automatic
// function returns a fixed array of records directly to generated child input
// ports, where each child checks every field of the aggregate value.
package generate_port_pkg;
    typedef struct {
        logic [7:0] value;
        bit valid;
    } entry_t;

    typedef entry_t payload_t [0:1];
endpackage

module payload_checker #(
    parameter int unsigned BASE = 0
)(
    input generate_port_pkg::payload_t payload,
    output logic ok
);
    initial begin
        if (payload[0].value !== BASE + 1 || payload[0].valid !== 1'b1 ||
            payload[1].value !== (BASE ^ 8'ha5) ||
            payload[1].valid !== (BASE[0] ? 1'b1 : 1'b0))
            $fatal(1, "generated child received wrong payload");
        ok = 1'b1;
    end
endmodule

module tb;
    import generate_port_pkg::*;

    logic [1:0] child_ok;

    function automatic payload_t make_payload(input int unsigned base);
        payload_t result;
        result[0].value = base[7:0] + 8'h01;
        result[0].valid = 1'b1;
        result[1].value = base[7:0] ^ 8'ha5;
        result[1].valid = base[0];
        make_payload = result;
    endfunction

    for (genvar g = 0; g < 2; g++) begin : generated
        payload_checker #(.BASE(g + 1)) u_payload (
            .payload(make_payload(g + 1)),
            .ok(child_ok[g])
        );
    end

    initial begin
        #1;
        if (child_ok !== 2'b11)
            $fatal(1, "generated child checks did not complete");
        $display("generated=%b", child_ok);
        $finish(0);
    end
endmodule
