from flask import Flask, request, render_template
from wakeonlan import send_magic_packet

app = Flask(__name__)

@app.route("/wol", methods=['GET', 'POST'])
def wol():
    addr = request.args.get('addr', '')
    message = ""

    if request.method == 'POST':
        send_magic_packet(addr)
        message = 'Sent magic packet'

    return render_template('wol.html', message=message, addr=addr)

if __name__ == "__main__":
    app.run(host='0.0.0.0')
