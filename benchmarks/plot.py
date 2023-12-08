import numpy as np
import matplotlib.pyplot as plt
from sklearn.linear_model import LinearRegression
import sys
import glob
import json
import os


# read the json file which starts with 'local' 'docker' and 'cairn'
def load_data(filename):
    iters = []
    times = []
    std_devs = []

    with open(filename) as f:
        data = json.load(f)
        for result in data['results']:
            iters.append(int(result['parameters']['iter']))
            times.append(float(result['mean']))
            std_devs.append(float(result['stddev']))

    return np.array(iters), np.array(times), np.array(std_devs)


def count_dirs(dir_path):
    return len([f for f in os.listdir(dir_path) if os.path.isdir(os.path.join(dir_path, f))])


# read the first command argument
result_dir = sys.argv[1]

for i in range(1, count_dirs(result_dir) + 1):
    local_json = glob.glob(f'{result_dir}/{i}/local*.json')
    docker_json = glob.glob(f'{result_dir}/{i}/docker*.json')
    cairn_json = glob.glob(f'{result_dir}/{i}/cairn*.json')

    x_source1, y_source1_mean, y_source1_std = load_data(local_json[0]) if local_json else ([], [], [])
    x_source2, y_source2_mean, y_source2_std = load_data(docker_json[0]) if docker_json else ([], [], [])
    x_source3, y_source3_mean, y_source3_std = load_data(cairn_json[0]) if cairn_json else ([], [], [])

    # Plot the data points with error bars representing standard deviation
    plt.errorbar(x_source1, y_source1_mean, yerr=y_source1_std, fmt='o', color='blue', label='local')
    plt.errorbar(x_source2, y_source2_mean, yerr=y_source2_std, fmt='o', color='green', label='docker')
    plt.errorbar(x_source3, y_source3_mean, yerr=y_source3_std, fmt='o', color='red', label='cairn')

    # Fit linear regression models
    model1 = LinearRegression().fit(x_source1.reshape(-1, 1), y_source1_mean)
    model2 = LinearRegression().fit(x_source2.reshape(-1, 1), y_source2_mean)
    model3 = LinearRegression().fit(x_source3.reshape(-1, 1), y_source3_mean)

    # Plot the regression lines
    plt.plot(x_source1, model1.predict(x_source1.reshape(-1, 1)), color='blue', linestyle='dashed', linewidth=2)
    plt.plot(x_source2, model2.predict(x_source2.reshape(-1, 1)), color='green', linestyle='dashed', linewidth=2)
    plt.plot(x_source3, model3.predict(x_source3.reshape(-1, 1)), color='red', linestyle='dashed', linewidth=2)

    # Add labels and legend
    plt.xlabel('Number of iterations')
    plt.ylabel('Time (seconds)')
    plt.legend()

    # Save the plot as a PNG file
    plt.savefig(f'{result_dir}/{i}/plot.png')

    # Show the plot
    # plt.show()
