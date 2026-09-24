// llg-test-fixture: IEEE 1800-2009 §§6.21, 13.5.2, 23.2.2.2, and 27.4.
// Generated instances use selected net-array elements for inout ports, while
// generated initial blocks pass distinct variable-array elements to ref and
// const-ref subroutine formals.
module generated_inout_leaf(inout wire [7:0] pin, input logic [7:0] drive);
    assign pin = drive;
endmodule

module tb;
    wire [7:0] pads [0:1];
    logic [7:0] drives [0:1];
    logic [7:0] ref_values [0:1];
    logic [7:0] const_values [0:1];
    logic [7:0] const_results [0:1];

    task automatic increment(ref logic [7:0] target, input logic [7:0] amount);
        target = target + amount;
    endtask

    function automatic logic [7:0] invert(const ref logic [7:0] source);
        invert = ~source;
    endfunction

    for (genvar g = 0; g < 2; g++) begin : generated
        generated_inout_leaf u_leaf(.pin(pads[g]), .drive(drives[g]));

        initial begin
            #1;
            increment(ref_values[g], 8'h01 << g);
            const_results[g] = invert(const_values[g]);
        end
    end

    initial begin
        drives[0] = 8'h3c;
        drives[1] = 8'ha5;
        ref_values[0] = 8'h11;
        ref_values[1] = 8'h22;
        const_values[0] = 8'haa;
        const_values[1] = 8'h3c;

        #2;
        if (pads[0] !== 8'h3c || pads[1] !== 8'ha5 ||
            ref_values[0] !== 8'h12 || ref_values[1] !== 8'h24 ||
            const_results[0] !== 8'h55 || const_results[1] !== 8'hc3)
            $fatal(1, "generated actual paths produced wrong values");
        $display("pads=%h,%h ref=%h,%h const=%h,%h",
                 pads[0], pads[1], ref_values[0], ref_values[1],
                 const_results[0], const_results[1]);
        $finish(0);
    end
endmodule
