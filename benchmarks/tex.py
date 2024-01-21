import glob
import sys
import json
import os


def load_data(filename):
    with open(filename) as f:
        data = json.load(f)
        return float(data['results'][0]['mean']) * 1000, float(data['results'][0]['stddev']) * 1000


def count_dirs(dir_path):
    return len([f for f in os.listdir(dir_path) if os.path.isdir(os.path.join(dir_path, f))])


def escape(string):
    return string.replace('_', r'\_')


result_dir = sys.argv[1]
names = ['local', 'docker', 'fuse_ll_docker', 'fuse_docker', 'cairn_I_', 'cairn_II_', 'cairn_III_', 'cairn_IV_', 'cairn_V_']

with open(f'{result_dir}/results.tex', 'w') as f:
    header = r"""
\begin{table}[H]
    \centering
    \begin{tabularx}{0.8\textwidth}{
        l
       *{4}{>{\centering\arraybackslash}X}
    }\toprule
    & {Local} & {Docker} & {Passthrough FUSE I} & {Passthrough FUSE II} & {Cairn I} & {Cairn II} & {Cairn II} & {Cairn III} & {Cairn IV} & {Cairn V}\\\midrule
"""
    footer = r"""    \bottomrule
    \end{tabularx}
    \caption{...}
    \label{tab:...}
\end{table}
"""

    f.write(header)

    for d in os.listdir(result_dir):
        if d == 'stress':
            continue

        path = os.path.join(result_dir, d)
        if os.path.isdir(path):
            rows = []
            for i in range(1, count_dirs(path) + 1):
                row = []
                jsons = []

                with open(f'{result_dir}/{d}/{i}/run.sh') as c:
                    row.append(f'\\texttt{{{escape(c.read().rstrip())}}}')

                for name in names:
                    jsons.append(glob.glob(f'{result_dir}/{d}/{i}/{name}*.json'))

                for j in jsons:
                    mean, std = load_data(j[0])
                    row.append(f'{int(mean)}({int(std)})')

                rows.append(row)

            for row in rows:
                f.write("    " + " & ".join(map(str, row)) + r" \\ " + "\n")

    f.write(footer)
