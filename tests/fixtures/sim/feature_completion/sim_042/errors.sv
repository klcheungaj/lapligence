// llg-test-fixture: SIM-042 A03 error behaviour. errors.c ($errors, $keep,
// $use_kept, $stress) checks that invalid, released and stale handles,
// unsupported properties and relations and illegal puts report a
// vpi_chk_error level with NULL, vpiUndefined or 0, never a guessed success.
// $stress churns handles for the sanitizer lane. Expected output: errors.out.
module tb;
    logic [7:0] v = 8'h5a;
    logic [7:0] mem [0:3];
    wire [7:0] n [0:1];
    real r = 1.0;
    localparam int P = 3;
    assign n[0] = v;
    initial begin
        $errors;
        $keep(v + 8'd1);
        $use_kept;
        $stress;
        $finish;
    end
endmodule
